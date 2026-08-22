use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use crossterm::cursor::MoveTo;
use crossterm::cursor::RestorePosition;
use crossterm::cursor::SavePosition;
use crossterm::queue;

use super::ImageProtocol;
use super::ImageSource;
use super::MediaNode;
use super::MediaPlacementUpdate;
use super::kitty_delete_image;
use super::kitty_transmit_png_file_with_id;
use super::kitty_transmit_png_with_id;
use super::resolve_image_source;
use ratatui::layout::Rect;

const PNG_SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MediaWriteReport {
    pub(crate) placed: usize,
    pub(crate) skipped: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedKittyPlacement {
    pub(crate) rect: Rect,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreparedKittyUpdate {
    pub(crate) deletions: Vec<String>,
    pub(crate) placements: Vec<PreparedKittyPlacement>,
    pub(crate) report: MediaWriteReport,
}

pub(crate) fn prepare_kitty_placement_update(
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
) -> Result<PreparedKittyUpdate> {
    if matches!(protocol, ImageProtocol::Sixel) {
        bail!("Kitty placement writer does not support Sixel");
    }

    let mut prepared = PreparedKittyUpdate {
        deletions: update
            .retired
            .iter()
            .map(|image_id| kitty_delete_image(*image_id))
            .collect(),
        ..PreparedKittyUpdate::default()
    };
    for placement in &update.placed {
        let source = match &placement.request.request.node {
            MediaNode::Image { source, .. } => source,
        };
        let Ok(ImageSource::Local(path)) = resolve_image_source(source) else {
            prepared.report.skipped += 1;
            continue;
        };
        if !has_png_signature(&path)? {
            prepared.report.skipped += 1;
            continue;
        }
        let rect = placement.request.request.rect;
        let command = match protocol {
            ImageProtocol::Kitty => {
                kitty_transmit_png_with_id(&path, rect.width, rect.height, Some(placement.id))?
            }
            ImageProtocol::KittyLocalFile => {
                kitty_transmit_png_file_with_id(&path, rect.width, rect.height, Some(placement.id))?
            }
            ImageProtocol::Sixel => unreachable!("Sixel rejected above"),
        };
        prepared
            .placements
            .push(PreparedKittyPlacement { rect, command });
        prepared.report.placed += 1;
    }
    Ok(prepared)
}

fn has_png_signature(path: &Path) -> Result<bool> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut signature = [0_u8; PNG_SIGNATURE.len()];
    match file.read_exact(&mut signature) {
        Ok(()) => Ok(signature == PNG_SIGNATURE),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("read PNG signature from {}", path.display()))
        }
    }
}

/// Emits one lifecycle update to an injected terminal sink.
///
/// Only static local PNG sources are handled here. Remote sources and invalid paths remain text
/// fallbacks until the bounded asynchronous loader exists. The request objects are read-only, so
/// terminal protocol bytes cannot leak back into Ratatui lines or persisted transcript source.
pub(crate) fn write_kitty_placement_update(
    writer: &mut impl Write,
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
) -> Result<MediaWriteReport> {
    let prepared = prepare_kitty_placement_update(protocol, update)?;
    for command in &prepared.deletions {
        writer.write_all(command.as_bytes())?;
    }
    for placement in &prepared.placements {
        queue!(
            writer,
            SavePosition,
            MoveTo(placement.rect.x, placement.rect.y)
        )?;
        writer.write_all(placement.command.as_bytes())?;
        queue!(writer, RestorePosition)?;
    }
    writer.flush()?;
    Ok(prepared.report)
}
