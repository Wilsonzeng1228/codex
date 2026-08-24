use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use ratex_layout::LayoutOptions;
use ratex_render::RenderOptions;
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use thiserror::Error;

use super::local_loader::LoadedLocalImage;

const DEFAULT_MAX_SOURCE_BYTES: usize = 4 * 1024;
const DEFAULT_MAX_DIMENSION: u32 = 4096;
const DEFAULT_MAX_PIXELS: u64 = 8 * 1024 * 1024;
const DEFAULT_MAX_PREPARED_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_CACHE_ENTRIES: usize = 64;
const DEFAULT_MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_RENDER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LatexRenderRequest {
    pub(crate) source: String,
    pub(crate) display: bool,
    pub(crate) width_cells: u16,
    pub(crate) foreground: (u8, u8, u8),
}

impl LatexRenderRequest {
    pub(crate) fn new(
        source: impl Into<String>,
        display: bool,
        width_cells: u16,
        foreground: (u8, u8, u8),
    ) -> Self {
        Self {
            source: source.into(),
            display,
            width_cells,
            foreground,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum LatexRenderError {
    #[error("LaTeX source is empty")]
    EmptySource,
    #[error("LaTeX source is too large ({size} bytes; max {max} bytes)")]
    SourceTooLarge { size: usize, max: usize },
    #[error("failed to parse LaTeX: {message}")]
    Parse { message: String },
    #[error("rendered LaTeX dimensions {width}x{height} exceed maximum {max_dimension}")]
    DimensionLimitExceeded {
        width: u32,
        height: u32,
        max_dimension: u32,
    },
    #[error("rendered LaTeX dimensions {width}x{height} exceed {max_pixels} pixels")]
    PixelLimitExceeded {
        width: u32,
        height: u32,
        max_pixels: u64,
    },
    #[error("failed to render LaTeX: {message}")]
    Render { message: String },
    #[error("rendered LaTeX PNG is too large ({size} bytes; max {max} bytes)")]
    PreparedTooLarge { size: usize, max: usize },
    #[error("LaTeX render timed out after {milliseconds} ms")]
    Timeout { milliseconds: u64 },
    #[error("LaTeX render worker failed: {message}")]
    Worker { message: String },
}

#[derive(Clone, Copy, Debug)]
struct LatexRenderLimits {
    max_source_bytes: usize,
    max_dimension: u32,
    max_pixels: u64,
    max_prepared_bytes: usize,
    max_cache_entries: usize,
    max_cache_bytes: usize,
    render_timeout: Duration,
}

impl Default for LatexRenderLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_dimension: DEFAULT_MAX_DIMENSION,
            max_pixels: DEFAULT_MAX_PIXELS,
            max_prepared_bytes: DEFAULT_MAX_PREPARED_BYTES,
            max_cache_entries: DEFAULT_MAX_CACHE_ENTRIES,
            max_cache_bytes: DEFAULT_MAX_CACHE_BYTES,
            render_timeout: DEFAULT_RENDER_TIMEOUT,
        }
    }
}

#[derive(Debug, Default)]
struct LatexRenderCache {
    entries: VecDeque<(LatexRenderRequest, LoadedLocalImage)>,
    bytes: usize,
}

impl LatexRenderCache {
    fn get(&mut self, request: &LatexRenderRequest) -> Option<LoadedLocalImage> {
        let index = self.entries.iter().position(|(key, _)| key == request)?;
        let entry = self.entries.remove(index)?;
        let image = entry.1.clone();
        self.entries.push_back(entry);
        Some(image)
    }

    fn insert(
        &mut self,
        request: LatexRenderRequest,
        image: LoadedLocalImage,
        limits: LatexRenderLimits,
    ) {
        if limits.max_cache_entries == 0
            || limits.max_cache_bytes == 0
            || image.bytes.len() > limits.max_cache_bytes
        {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(key, _)| key == &request)
            && let Some((_, replaced)) = self.entries.remove(index)
        {
            self.bytes = self.bytes.saturating_sub(replaced.bytes.len());
        }
        self.bytes = self.bytes.saturating_add(image.bytes.len());
        self.entries.push_back((request, image));
        while self.entries.len() > limits.max_cache_entries || self.bytes > limits.max_cache_bytes {
            let Some((_, evicted)) = self.entries.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(evicted.bytes.len());
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LatexRenderer {
    limits: LatexRenderLimits,
    cache: Arc<Mutex<LatexRenderCache>>,
}

impl LatexRenderer {
    pub(crate) async fn render(
        &self,
        request: LatexRenderRequest,
    ) -> Result<LoadedLocalImage, LatexRenderError> {
        validate_source(&request, self.limits)?;
        if let Some(image) = lock_cache(&self.cache).get(&request) {
            return Ok(image);
        }

        let limits = self.limits;
        let worker_request = request.clone();
        let task = tokio::task::spawn_blocking(move || render_formula(&worker_request, limits));
        let image = match tokio::time::timeout(limits.render_timeout, task).await {
            Ok(Ok(result)) => result?,
            Ok(Err(error)) => {
                return Err(LatexRenderError::Worker {
                    message: error.to_string(),
                });
            }
            Err(_) => {
                return Err(LatexRenderError::Timeout {
                    milliseconds: limits
                        .render_timeout
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX),
                });
            }
        };
        lock_cache(&self.cache).insert(request, image.clone(), limits);
        Ok(image)
    }

    #[cfg(test)]
    pub(crate) fn cache_len_for_test(&self) -> usize {
        lock_cache(&self.cache).entries.len()
    }
}

fn lock_cache(cache: &Mutex<LatexRenderCache>) -> std::sync::MutexGuard<'_, LatexRenderCache> {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn validate_source(
    request: &LatexRenderRequest,
    limits: LatexRenderLimits,
) -> Result<(), LatexRenderError> {
    if request.source.is_empty() {
        return Err(LatexRenderError::EmptySource);
    }
    if request.source.len() > limits.max_source_bytes {
        return Err(LatexRenderError::SourceTooLarge {
            size: request.source.len(),
            max: limits.max_source_bytes,
        });
    }
    Ok(())
}

fn render_formula(
    request: &LatexRenderRequest,
    limits: LatexRenderLimits,
) -> Result<LoadedLocalImage, LatexRenderError> {
    let ast = ratex_parser::parse(&request.source).map_err(|error| LatexRenderError::Parse {
        message: error.to_string(),
    })?;
    let (red, green, blue) = request.foreground;
    let color = Color::rgb(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
    );
    let layout = ratex_layout::layout(
        &ast,
        &LayoutOptions {
            style: if request.display {
                MathStyle::Display
            } else {
                MathStyle::Text
            },
            color,
            ..LayoutOptions::default()
        },
    );
    let display_list = ratex_layout::to_display_list(&layout);
    let font_size = if request.display { 40.0 } else { 32.0 };
    let padding = 4.0;
    let width = checked_dimension(display_list.width * font_size + 2.0 * padding)?;
    let height =
        checked_dimension((display_list.height + display_list.depth) * font_size + 2.0 * padding)?;
    if width > limits.max_dimension || height > limits.max_dimension {
        return Err(LatexRenderError::DimensionLimitExceeded {
            width,
            height,
            max_dimension: limits.max_dimension,
        });
    }
    if u64::from(width).saturating_mul(u64::from(height)) > limits.max_pixels {
        return Err(LatexRenderError::PixelLimitExceeded {
            width,
            height,
            max_pixels: limits.max_pixels,
        });
    }

    let bytes = ratex_render::render_to_png(
        &display_list,
        &RenderOptions {
            font_size: font_size as f32,
            padding: padding as f32,
            background_color: Color::new(0.0, 0.0, 0.0, 0.0),
            font_dir: String::new(),
            device_pixel_ratio: 1.0,
        },
    )
    .map_err(|message| LatexRenderError::Render { message })?;
    if bytes.len() > limits.max_prepared_bytes {
        return Err(LatexRenderError::PreparedTooLarge {
            size: bytes.len(),
            max: limits.max_prepared_bytes,
        });
    }
    Ok(LoadedLocalImage {
        bytes: Arc::from(bytes),
        source_width: width,
        source_height: height,
        width,
        height,
        can_use_source_file: false,
    })
}

fn checked_dimension(value: f64) -> Result<u32, LatexRenderError> {
    if !value.is_finite() || value <= 0.0 || value.ceil() > f64::from(u32::MAX) {
        return Err(LatexRenderError::Render {
            message: "renderer produced invalid dimensions".to_string(),
        });
    }
    Ok(value.ceil() as u32)
}
