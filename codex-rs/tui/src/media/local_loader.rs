use std::collections::VecDeque;
use std::fs::File;
use std::io::Cursor;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

use image::DynamicImage;
use image::ImageDecoder;
use image::ImageFormat;
use image::ImageReader;
use image::imageops::FilterType;
use thiserror::Error;

const DEFAULT_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_DIMENSION: u32 = 8192;
const DEFAULT_MAX_DECODED_PIXELS: u64 = 4 * 1024 * 1024;
const DEFAULT_MAX_OUTPUT_DIMENSION: u32 = 2048;
const DEFAULT_MAX_PREPARED_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_CACHE_ENTRIES: usize = 32;
const DEFAULT_MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_LOAD_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalImageLimits {
    pub(crate) max_source_bytes: usize,
    pub(crate) max_dimension: u32,
    pub(crate) max_decoded_pixels: u64,
    pub(crate) max_output_dimension: u32,
    pub(crate) max_prepared_bytes: usize,
    pub(crate) max_cache_entries: usize,
    pub(crate) max_cache_bytes: usize,
    pub(crate) load_timeout: Duration,
}

impl Default for LocalImageLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_dimension: DEFAULT_MAX_DIMENSION,
            max_decoded_pixels: DEFAULT_MAX_DECODED_PIXELS,
            max_output_dimension: DEFAULT_MAX_OUTPUT_DIMENSION,
            max_prepared_bytes: DEFAULT_MAX_PREPARED_BYTES,
            max_cache_entries: DEFAULT_MAX_CACHE_ENTRIES,
            max_cache_bytes: DEFAULT_MAX_CACHE_BYTES,
            load_timeout: DEFAULT_LOAD_TIMEOUT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalImageRenderParams {
    max_width: u32,
    max_height: u32,
}

impl LocalImageRenderParams {
    pub(crate) fn new(max_width: u32, max_height: u32) -> Self {
        Self {
            max_width: max_width.max(1),
            max_height: max_height.max(1),
        }
    }

    fn constrained_by(self, max_dimension: u32) -> Self {
        let max_dimension = max_dimension.max(1);
        Self::new(
            self.max_width.min(max_dimension),
            self.max_height.min(max_dimension),
        )
    }
}

impl Default for LocalImageRenderParams {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_OUTPUT_DIMENSION, DEFAULT_MAX_OUTPUT_DIMENSION)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedLocalImage {
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) source_width: u32,
    pub(crate) source_height: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) can_use_source_file: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum LocalImageLoadError {
    #[error("failed to {operation} local image {path}: {message}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error("local image source is too large ({size} bytes; max {max} bytes)")]
    SourceTooLarge { size: usize, max: usize },
    #[error("local image is not a PNG: {message}")]
    InvalidPng { message: String },
    #[error("local image format is not supported: {format}")]
    UnsupportedFormat { format: String },
    #[error("local image is invalid: {message}")]
    InvalidImage { message: String },
    #[error("local image dimensions {width}x{height} exceed maximum {max_dimension}")]
    DimensionLimitExceeded {
        width: u32,
        height: u32,
        max_dimension: u32,
    },
    #[error("local image dimensions {width}x{height} exceed {max_pixels} decoded pixels")]
    PixelLimitExceeded {
        width: u32,
        height: u32,
        max_pixels: u64,
    },
    #[error("failed to decode image {source_label}: {message}")]
    Decode {
        source_label: String,
        message: String,
    },
    #[error("failed to encode prepared local PNG: {message}")]
    Encode { message: String },
    #[error("prepared local PNG is too large ({size} bytes; max {max} bytes)")]
    PreparedTooLarge { size: usize, max: usize },
    #[error("local image load timed out after {milliseconds} ms")]
    Timeout { milliseconds: u64 },
    #[error("local image worker failed: {message}")]
    Worker { message: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceFileReuse {
    Available,
    // The remote policy foundation is not connected to a production HTTP adapter yet.
    #[allow(dead_code)]
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalImageCacheKey {
    path: PathBuf,
    file_len: u64,
    modified: Option<SystemTime>,
    render_params: LocalImageRenderParams,
}

#[derive(Debug, Default)]
struct LocalImageCache {
    entries: VecDeque<(LocalImageCacheKey, LoadedLocalImage)>,
    bytes: usize,
}

impl LocalImageCache {
    fn get(&mut self, key: &LocalImageCacheKey) -> Option<LoadedLocalImage> {
        let index = self.entries.iter().position(|(entry, _)| entry == key)?;
        let entry = self.entries.remove(index)?;
        let image = entry.1.clone();
        self.entries.push_back(entry);
        Some(image)
    }

    fn insert(
        &mut self,
        key: LocalImageCacheKey,
        image: LoadedLocalImage,
        limits: LocalImageLimits,
    ) {
        if limits.max_cache_entries == 0
            || limits.max_cache_bytes == 0
            || image.bytes.len() > limits.max_cache_bytes
        {
            return;
        }

        if let Some(index) = self.entries.iter().position(|(entry, _)| entry == &key)
            && let Some((_, replaced)) = self.entries.remove(index)
        {
            self.bytes = self.bytes.saturating_sub(replaced.bytes.len());
        }
        self.bytes = self.bytes.saturating_add(image.bytes.len());
        self.entries.push_back((key, image));

        while self.entries.len() > limits.max_cache_entries || self.bytes > limits.max_cache_bytes {
            let Some((_, evicted)) = self.entries.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(evicted.bytes.len());
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LocalImageLoader {
    limits: LocalImageLimits,
    cache: Arc<Mutex<LocalImageCache>>,
}

impl LocalImageLoader {
    pub(crate) fn new(limits: LocalImageLimits) -> Self {
        Self {
            limits,
            cache: Arc::new(Mutex::new(LocalImageCache::default())),
        }
    }

    pub(crate) fn clear_cache(&self) {
        *cache_lock(&self.cache) = LocalImageCache::default();
    }

    /// Load a supported local image and prepare a static PNG without blocking the executor thread.
    pub(crate) async fn load_image(
        &self,
        path: &Path,
        render_params: LocalImageRenderParams,
    ) -> Result<LoadedLocalImage, LocalImageLoadError> {
        let path = path.to_path_buf();
        let limits = self.limits;
        let render_params = render_params.constrained_by(limits.max_output_dimension);
        let cache = Arc::clone(&self.cache);
        let task =
            tokio::task::spawn_blocking(move || load_image(path, limits, render_params, &cache));
        match tokio::time::timeout(limits.load_timeout, task).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => Err(LocalImageLoadError::Worker {
                message: error.to_string(),
            }),
            Err(_) => Err(LocalImageLoadError::Timeout {
                milliseconds: limits
                    .load_timeout
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            }),
        }
    }
}

fn load_image(
    path: PathBuf,
    limits: LocalImageLimits,
    render_params: LocalImageRenderParams,
    cache: &Mutex<LocalImageCache>,
) -> Result<LoadedLocalImage, LocalImageLoadError> {
    let canonical_path = std::fs::canonicalize(&path).map_err(|error| LocalImageLoadError::Io {
        operation: "canonicalize",
        path: path.clone(),
        message: error.to_string(),
    })?;
    let mut file = File::open(&canonical_path).map_err(|error| LocalImageLoadError::Io {
        operation: "open",
        path: canonical_path.clone(),
        message: error.to_string(),
    })?;
    let metadata = file.metadata().map_err(|error| LocalImageLoadError::Io {
        operation: "inspect",
        path: canonical_path.clone(),
        message: error.to_string(),
    })?;
    let source_size = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if source_size > limits.max_source_bytes {
        return Err(LocalImageLoadError::SourceTooLarge {
            size: source_size,
            max: limits.max_source_bytes,
        });
    }
    let key = LocalImageCacheKey {
        path: canonical_path.clone(),
        file_len: metadata.len(),
        modified: metadata.modified().ok(),
        render_params,
    };
    if let Some(image) = cache_lock(cache).get(&key) {
        return Ok(image);
    }

    let read_limit = u64::try_from(limits.max_source_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(source_size);
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| LocalImageLoadError::Io {
            operation: "read",
            path: canonical_path.clone(),
            message: error.to_string(),
        })?;
    if bytes.len() > limits.max_source_bytes {
        return Err(LocalImageLoadError::SourceTooLarge {
            size: bytes.len(),
            max: limits.max_source_bytes,
        });
    }

    let image = prepare_image_bytes(
        bytes,
        limits,
        render_params,
        canonical_path.display().to_string(),
        SourceFileReuse::Available,
    )?;
    cache_lock(cache).insert(key, image.clone(), limits);
    Ok(image)
}

pub(super) fn prepare_image_bytes(
    bytes: Vec<u8>,
    limits: LocalImageLimits,
    render_params: LocalImageRenderParams,
    source: String,
    source_file_reuse: SourceFileReuse,
) -> Result<LoadedLocalImage, LocalImageLoadError> {
    if bytes.len() > limits.max_source_bytes {
        return Err(LocalImageLoadError::SourceTooLarge {
            size: bytes.len(),
            max: limits.max_source_bytes,
        });
    }
    let render_params = render_params.constrained_by(limits.max_output_dimension);
    let format =
        image::guess_format(&bytes).map_err(|error| LocalImageLoadError::InvalidImage {
            message: error.to_string(),
        })?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP
    ) {
        return Err(LocalImageLoadError::UnsupportedFormat {
            format: format!("{format:?}"),
        });
    }
    let decoder = ImageReader::with_format(Cursor::new(bytes.as_slice()), format)
        .into_decoder()
        .map_err(|error| {
            if format == ImageFormat::Png {
                LocalImageLoadError::InvalidPng {
                    message: error.to_string(),
                }
            } else {
                LocalImageLoadError::InvalidImage {
                    message: error.to_string(),
                }
            }
        })?;
    let (source_width, source_height) = decoder.dimensions();
    if source_width > limits.max_dimension || source_height > limits.max_dimension {
        return Err(LocalImageLoadError::DimensionLimitExceeded {
            width: source_width,
            height: source_height,
            max_dimension: limits.max_dimension,
        });
    }
    let decoded_pixels = u64::from(source_width) * u64::from(source_height);
    if decoded_pixels > limits.max_decoded_pixels {
        return Err(LocalImageLoadError::PixelLimitExceeded {
            width: source_width,
            height: source_height,
            max_pixels: limits.max_decoded_pixels,
        });
    }

    let decoded =
        DynamicImage::from_decoder(decoder).map_err(|error| LocalImageLoadError::Decode {
            source_label: source,
            message: error.to_string(),
        })?;
    let needs_resize =
        source_width > render_params.max_width || source_height > render_params.max_height;
    let (prepared_bytes, width, height) = if needs_resize || format != ImageFormat::Png {
        let prepared = if needs_resize {
            decoded.resize(
                render_params.max_width,
                render_params.max_height,
                FilterType::Triangle,
            )
        } else {
            decoded
        };
        let mut encoded = Cursor::new(Vec::new());
        prepared
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|error| LocalImageLoadError::Encode {
                message: error.to_string(),
            })?;
        (encoded.into_inner(), prepared.width(), prepared.height())
    } else {
        (bytes, source_width, source_height)
    };
    if prepared_bytes.len() > limits.max_prepared_bytes {
        return Err(LocalImageLoadError::PreparedTooLarge {
            size: prepared_bytes.len(),
            max: limits.max_prepared_bytes,
        });
    }

    let image = LoadedLocalImage {
        bytes: prepared_bytes.into(),
        source_width,
        source_height,
        width,
        height,
        can_use_source_file: source_file_reuse == SourceFileReuse::Available
            && format == ImageFormat::Png
            && !needs_resize,
    };
    Ok(image)
}

fn cache_lock(cache: &Mutex<LocalImageCache>) -> std::sync::MutexGuard<'_, LocalImageCache> {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
