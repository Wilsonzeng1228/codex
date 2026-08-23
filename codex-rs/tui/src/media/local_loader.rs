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
use image::codecs::png::PngDecoder;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoadedLocalImage {
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) source_width: u32,
    pub(crate) source_height: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
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
    #[error("failed to decode local PNG {path}: {message}")]
    Decode { path: PathBuf, message: String },
    #[error("failed to encode prepared local PNG: {message}")]
    Encode { message: String },
    #[error("prepared local PNG is too large ({size} bytes; max {max} bytes)")]
    PreparedTooLarge { size: usize, max: usize },
    #[error("local image load timed out after {milliseconds} ms")]
    Timeout { milliseconds: u64 },
    #[error("local image worker failed: {message}")]
    Worker { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalImageCacheKey {
    path: PathBuf,
    file_len: u64,
    modified: Option<SystemTime>,
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

    /// Load and prepare a PNG without blocking the async caller's executor thread.
    pub(crate) async fn load_png(
        &self,
        path: &Path,
    ) -> Result<LoadedLocalImage, LocalImageLoadError> {
        let path = path.to_path_buf();
        let limits = self.limits;
        let cache = Arc::clone(&self.cache);
        let task = tokio::task::spawn_blocking(move || load_png(path, limits, &cache));
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

fn load_png(
    path: PathBuf,
    limits: LocalImageLimits,
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

    let decoder = PngDecoder::new(Cursor::new(bytes.as_slice())).map_err(|error| {
        LocalImageLoadError::InvalidPng {
            message: error.to_string(),
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
            path: canonical_path.clone(),
            message: error.to_string(),
        })?;
    let (prepared_bytes, width, height) = if source_width > limits.max_output_dimension
        || source_height > limits.max_output_dimension
    {
        let resized = decoded.resize(
            limits.max_output_dimension.max(1),
            limits.max_output_dimension.max(1),
            FilterType::Triangle,
        );
        let mut encoded = Cursor::new(Vec::new());
        resized
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|error| LocalImageLoadError::Encode {
                message: error.to_string(),
            })?;
        (encoded.into_inner(), resized.width(), resized.height())
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
    };
    cache_lock(cache).insert(key, image.clone(), limits);
    Ok(image)
}

fn cache_lock(cache: &Mutex<LocalImageCache>) -> std::sync::MutexGuard<'_, LocalImageCache> {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
