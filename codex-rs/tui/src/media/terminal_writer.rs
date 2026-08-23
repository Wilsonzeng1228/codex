use std::io::Write;

use anyhow::Result;
use anyhow::bail;
use crossterm::cursor::MoveTo;
use crossterm::cursor::RestorePosition;
use crossterm::cursor::SavePosition;
use crossterm::queue;

use super::ImageProtocol;
use super::ImageSource;
use super::MediaImageState;
use super::MediaNode;
use super::MediaPlacementUpdate;
use super::image::iterm2_transmit_png_bytes;
use super::image::kitty_transmit_png_bytes_with_id;
use super::kitty_delete_image;
use super::kitty_transmit_png_file_with_id;
use super::resolve_image_source;
use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MediaWriteReport {
    pub(crate) placed: usize,
    pub(crate) pending: usize,
    pub(crate) skipped: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedMediaPlacement {
    pub(crate) rect: Rect,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreparedMediaUpdate {
    pub(crate) deletions: Vec<String>,
    pub(crate) placements: Vec<PreparedMediaPlacement>,
    pub(crate) report: MediaWriteReport,
}

pub(crate) fn prepare_media_placement_update(
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
    mut image_state: impl FnMut(&super::placement::RegisteredMediaPlacement) -> MediaImageState,
) -> Result<PreparedMediaUpdate> {
    if matches!(protocol, ImageProtocol::Sixel) {
        bail!("terminal placement writer does not support Sixel");
    }

    let mut prepared = PreparedMediaUpdate {
        deletions: if matches!(
            protocol,
            ImageProtocol::Kitty | ImageProtocol::KittyLocalFile
        ) {
            update
                .retired
                .iter()
                .map(|image_id| kitty_delete_image(*image_id))
                .collect()
        } else {
            Vec::new()
        },
        ..PreparedMediaUpdate::default()
    };
    for placement in &update.placed {
        let source = match &placement.request.request.node {
            MediaNode::Image { source, .. } => source,
        };
        let Ok(ImageSource::Local(path)) = resolve_image_source(source) else {
            prepared.report.skipped += 1;
            continue;
        };
        let loaded = match image_state(placement) {
            MediaImageState::Ready(loaded) => loaded,
            MediaImageState::Pending => {
                prepared.report.pending += 1;
                continue;
            }
            MediaImageState::Unavailable => {
                prepared.report.skipped += 1;
                continue;
            }
        };
        let rect = placement.request.request.rect;
        let command = match protocol {
            ImageProtocol::Iterm2Inline => {
                iterm2_transmit_png_bytes(&loaded.bytes, rect.width, rect.height)
            }
            ImageProtocol::Kitty => kitty_transmit_png_bytes_with_id(
                &loaded.bytes,
                rect.width,
                rect.height,
                Some(placement.id),
            )?,
            ImageProtocol::KittyLocalFile => {
                if loaded.can_use_source_file {
                    kitty_transmit_png_file_with_id(
                        &path,
                        rect.width,
                        rect.height,
                        Some(placement.id),
                    )?
                } else {
                    kitty_transmit_png_bytes_with_id(
                        &loaded.bytes,
                        rect.width,
                        rect.height,
                        Some(placement.id),
                    )?
                }
            }
            ImageProtocol::Sixel => unreachable!("Sixel rejected above"),
        };
        prepared
            .placements
            .push(PreparedMediaPlacement { rect, command });
        prepared.report.placed += 1;
    }
    Ok(prepared)
}

/// Emits one lifecycle update to an injected terminal sink.
///
/// 当前处理静态本地 PNG/JPEG/WebP 和 GIF 首帧。文件先经过有界解码、缩放和缓存准备；远程来源或无效路径继续
/// 使用文本降级。请求对象保持只读，因此终端协议字节不会回写 Ratatui 行或持久化 transcript。
pub(crate) fn write_media_placement_update(
    writer: &mut impl Write,
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
    image_state: impl FnMut(&super::placement::RegisteredMediaPlacement) -> MediaImageState,
) -> Result<MediaWriteReport> {
    let prepared = prepare_media_placement_update(protocol, update, image_state)?;
    write_prepared_media_update(writer, &prepared)?;
    Ok(prepared.report)
}

pub(crate) fn write_prepared_media_update(
    writer: &mut impl Write,
    prepared: &PreparedMediaUpdate,
) -> Result<()> {
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
    Ok(())
}
