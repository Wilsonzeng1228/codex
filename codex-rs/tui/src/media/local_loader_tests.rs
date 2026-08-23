use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use image::DynamicImage;
use pretty_assertions::assert_eq;

use super::local_loader::LocalImageLimits;
use super::local_loader::LocalImageLoadError;
use super::local_loader::LocalImageLoader;

fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::new_rgba8(width, height)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("encode PNG fixture");
    encoded.into_inner()
}

fn limits() -> LocalImageLimits {
    LocalImageLimits {
        max_source_bytes: 1024 * 1024,
        max_dimension: 1024,
        max_decoded_pixels: 1024 * 1024,
        max_output_dimension: 2,
        max_prepared_bytes: 1024 * 1024,
        max_cache_entries: 1,
        max_cache_bytes: 1024 * 1024,
        load_timeout: Duration::from_secs(1),
    }
}

#[tokio::test]
async fn local_png_loader_decodes_resizes_and_reuses_cached_result() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("diagram.png");
    std::fs::write(&path, png_fixture(/*width*/ 4, /*height*/ 2)).expect("write PNG fixture");
    let loader = LocalImageLoader::new(limits());

    let first = loader.load_png(&path).await.expect("load local PNG");
    let cached = loader.load_png(&path).await.expect("load cached local PNG");

    assert_eq!((first.source_width, first.source_height), (4, 2));
    assert_eq!((first.width, first.height), (2, 1));
    assert!(first.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(Arc::ptr_eq(&first.bytes, &cached.bytes));
}

#[tokio::test]
async fn local_png_loader_rejects_source_and_decoded_pixel_limits() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("diagram.png");
    let fixture = png_fixture(/*width*/ 4, /*height*/ 2);
    std::fs::write(&path, &fixture).expect("write PNG fixture");

    let mut byte_limits = limits();
    byte_limits.max_source_bytes = fixture.len() - 1;
    let byte_error = LocalImageLoader::new(byte_limits)
        .load_png(&path)
        .await
        .expect_err("reject oversized source bytes");
    assert_eq!(
        byte_error,
        LocalImageLoadError::SourceTooLarge {
            size: fixture.len(),
            max: fixture.len() - 1,
        }
    );

    let mut pixel_limits = limits();
    pixel_limits.max_decoded_pixels = 7;
    let pixel_error = LocalImageLoader::new(pixel_limits)
        .load_png(&path)
        .await
        .expect_err("reject oversized decoded dimensions");
    assert_eq!(
        pixel_error,
        LocalImageLoadError::PixelLimitExceeded {
            width: 4,
            height: 2,
            max_pixels: 7,
        }
    );
}

#[tokio::test]
async fn local_png_loader_evicts_least_recently_used_entry() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let first_path = dir.path().join("first.png");
    let second_path = dir.path().join("second.png");
    std::fs::write(&first_path, png_fixture(/*width*/ 2, /*height*/ 2))
        .expect("write first PNG fixture");
    std::fs::write(&second_path, png_fixture(/*width*/ 1, /*height*/ 1))
        .expect("write second PNG fixture");
    let loader = LocalImageLoader::new(limits());

    let first = loader.load_png(&first_path).await.expect("load first PNG");
    loader
        .load_png(&second_path)
        .await
        .expect("load second PNG");
    let reloaded = loader
        .load_png(&first_path)
        .await
        .expect("reload evicted first PNG");

    assert!(!Arc::ptr_eq(&first.bytes, &reloaded.bytes));
}
