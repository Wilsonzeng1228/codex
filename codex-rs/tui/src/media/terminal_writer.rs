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
use super::image::iterm2_transmit_png_bytes_pixels;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalCellPixels {
    width: u16,
    height: u16,
}

impl TerminalCellPixels {
    pub(crate) fn new(width: u16, height: u16) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }

    pub(crate) fn from_window_size(
        columns_rows: ratatui::layout::Size,
        pixels: ratatui::layout::Size,
    ) -> Self {
        let width = pixels
            .width
            .checked_div(columns_rows.width)
            .unwrap_or_default();
        let height = pixels
            .height
            .checked_div(columns_rows.height)
            .unwrap_or_default();
        if width == 0 || height == 0 {
            Self::default()
        } else {
            Self::new(width, height)
        }
    }
}

impl Default for TerminalCellPixels {
    fn default() -> Self {
        Self::new(/*width*/ 8, /*height*/ 16)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedMediaPlacement {
    pub(crate) rect: Rect,
    pub(crate) command: String,
    pub(crate) clear_cells: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreparedMediaUpdate {
    pub(crate) deletions: Vec<String>,
    pub(crate) placements: Vec<PreparedMediaPlacement>,
    pub(crate) report: MediaWriteReport,
}

pub(crate) fn prepare_media_placement_update_with_cell_pixels(
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
    cell_pixels: TerminalCellPixels,
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
        let clear_cells = matches!(placement.request.request.node, MediaNode::Latex { .. });
        let source = match &placement.request.request.node {
            MediaNode::Image { source, .. } => match resolve_image_source(source) {
                Ok(source) => Some(source),
                Err(_) => {
                    prepared.report.skipped += 1;
                    continue;
                }
            },
            MediaNode::Latex { .. } => None,
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
            ImageProtocol::Iterm2Inline => match &placement.request.request.node {
                MediaNode::Image { .. } => {
                    let (width, height) = fitted_media_pixel_dimensions(&loaded, rect, cell_pixels);
                    iterm2_transmit_png_bytes_pixels(&loaded.bytes, width, height)
                }
                MediaNode::Latex { .. } => {
                    let (width, height) = fitted_media_pixel_dimensions(&loaded, rect, cell_pixels);
                    iterm2_transmit_png_bytes_pixels(&loaded.bytes, width, height)
                }
            },
            ImageProtocol::Kitty => kitty_transmit_png_bytes_with_id(
                &loaded.bytes,
                rect.width,
                rect.height,
                Some(placement.id),
            )?,
            ImageProtocol::KittyLocalFile => {
                if let Some(ImageSource::Local(path)) = &source
                    && loaded.can_use_source_file
                {
                    kitty_transmit_png_file_with_id(
                        path,
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
        prepared.placements.push(PreparedMediaPlacement {
            rect,
            command,
            clear_cells,
        });
        prepared.report.placed += 1;
    }
    Ok(prepared)
}

/// Emits one lifecycle update to an injected terminal sink.
///
/// 当前处理静态本地 PNG/JPEG/WebP、GIF 首帧和经过安全下载的 HTTPS 图片。来源先经过有界解码与缩放；
/// 无效或未准备好的来源继续使用文本降级。请求对象保持只读，因此终端协议字节不会回写 Ratatui 行或持久化 transcript。
#[cfg(test)]
pub(crate) fn write_media_placement_update(
    writer: &mut impl Write,
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
    image_state: impl FnMut(&super::placement::RegisteredMediaPlacement) -> MediaImageState,
) -> Result<MediaWriteReport> {
    write_media_placement_update_with_cell_pixels(
        writer,
        protocol,
        update,
        TerminalCellPixels::default(),
        image_state,
    )
}

pub(crate) fn write_media_placement_update_with_cell_pixels(
    writer: &mut impl Write,
    protocol: ImageProtocol,
    update: &MediaPlacementUpdate,
    cell_pixels: TerminalCellPixels,
    image_state: impl FnMut(&super::placement::RegisteredMediaPlacement) -> MediaImageState,
) -> Result<MediaWriteReport> {
    let prepared = prepare_media_placement_update_with_cell_pixels(
        protocol,
        update,
        cell_pixels,
        image_state,
    )?;
    write_prepared_media_update(writer, &prepared)?;
    Ok(prepared.report)
}

fn fitted_media_pixel_dimensions(
    loaded: &super::local_loader::LoadedLocalImage,
    rect: Rect,
    cell_pixels: TerminalCellPixels,
) -> (u32, u32) {
    let source_width = loaded.width.max(1);
    let source_height = loaded.height.max(1);
    let max_width = u32::from(rect.width.max(1)) * u32::from(cell_pixels.width);
    let max_height = u32::from(rect.height.max(1)) * u32::from(cell_pixels.height);

    // Contain the source inside the reserved cells while preserving its aspect ratio. Explicit
    // pixel dimensions keep WezTerm from choosing one cell axis and overflowing the other; they
    // also let small RaTeX bitmaps scale up to the readable formula area.
    if u64::from(source_width) * u64::from(max_height)
        >= u64::from(source_height) * u64::from(max_width)
    {
        let height = (u64::from(source_height) * u64::from(max_width) / u64::from(source_width))
            .max(1) as u32;
        (max_width, height)
    } else {
        let width = (u64::from(source_width) * u64::from(max_height) / u64::from(source_height))
            .max(1) as u32;
        (width, max_height)
    }
}

pub(crate) fn write_prepared_media_update(
    writer: &mut impl Write,
    prepared: &PreparedMediaUpdate,
) -> Result<()> {
    for command in &prepared.deletions {
        writer.write_all(command.as_bytes())?;
    }
    for placement in &prepared.placements {
        queue!(writer, SavePosition)?;
        if placement.clear_cells {
            let blank_row = " ".repeat(usize::from(placement.rect.width));
            for row in 0..placement.rect.height {
                queue!(
                    writer,
                    MoveTo(placement.rect.x, placement.rect.y.saturating_add(row))
                )?;
                writer.write_all(blank_row.as_bytes())?;
            }
        }
        queue!(writer, MoveTo(placement.rect.x, placement.rect.y))?;
        writer.write_all(placement.command.as_bytes())?;
        queue!(writer, RestorePosition)?;
    }
    writer.flush()?;
    Ok(())
}
