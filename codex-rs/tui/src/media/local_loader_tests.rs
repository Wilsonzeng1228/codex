use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use image::DynamicImage;
use image::Frame;
use image::GenericImageView;
use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;
use image::codecs::gif::GifEncoder;
use pretty_assertions::assert_eq;

use super::local_loader::LocalImageLimits;
use super::local_loader::LocalImageLoadError;
use super::local_loader::LocalImageLoader;
use super::local_loader::LocalImageRenderParams;

fn png_fixture(width: u32, height: u32) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::new_rgba8(width, height)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("encode PNG fixture");
    encoded.into_inner()
}

fn image_fixture(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        Rgba([20, 80, 160, 255]),
    ))
    .write_to(&mut encoded, format)
    .expect("encode image fixture");
    encoded.into_inner()
}

fn animated_gif_fixture() -> Vec<u8> {
    let mut encoded = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut encoded);
        encoder
            .encode_frame(Frame::new(RgbaImage::from_pixel(
                /*width*/ 2,
                /*height*/ 1,
                Rgba([255, 0, 0, 255]),
            )))
            .expect("encode first GIF frame");
        encoder
            .encode_frame(Frame::new(RgbaImage::from_pixel(
                /*width*/ 2,
                /*height*/ 1,
                Rgba([0, 0, 255, 255]),
            )))
            .expect("encode second GIF frame");
    }
    encoded
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

fn render_params() -> LocalImageRenderParams {
    LocalImageRenderParams::new(/*max_width*/ 2, /*max_height*/ 2)
}

#[tokio::test]
async fn local_png_loader_decodes_resizes_and_reuses_cached_result() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("diagram.png");
    std::fs::write(&path, png_fixture(/*width*/ 4, /*height*/ 2)).expect("write PNG fixture");
    let loader = LocalImageLoader::new(limits());

    let first = loader
        .load_image(&path, render_params())
        .await
        .expect("load local PNG");
    let cached = loader
        .load_image(&path, render_params())
        .await
        .expect("load cached local PNG");

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
        .load_image(&path, render_params())
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
        .load_image(&path, render_params())
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

    let first = loader
        .load_image(&first_path, render_params())
        .await
        .expect("load first PNG");
    loader
        .load_image(&second_path, render_params())
        .await
        .expect("load second PNG");
    let reloaded = loader
        .load_image(&first_path, render_params())
        .await
        .expect("reload evicted first PNG");

    assert!(!Arc::ptr_eq(&first.bytes, &reloaded.bytes));
}

#[tokio::test]
async fn local_png_loader_rejects_malformed_body_after_signature() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("truncated.png");
    std::fs::write(
        &path,
        [b"\x89PNG\r\n\x1a\n".as_slice(), b"not-a-png-body"].concat(),
    )
    .expect("write malformed PNG fixture");
    let error = LocalImageLoader::new(limits())
        .load_image(&path, render_params())
        .await
        .expect_err("reject malformed PNG body");

    assert!(matches!(error, LocalImageLoadError::InvalidPng { .. }));
}

#[tokio::test]
async fn local_image_loader_decodes_supported_formats_and_uses_first_gif_frame() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let cases = [
        (
            "diagram.jpg",
            image_fixture(3, 2, ImageFormat::Jpeg),
            (3, 2),
        ),
        (
            "diagram.webp",
            image_fixture(3, 2, ImageFormat::WebP),
            (3, 2),
        ),
        ("diagram.gif", animated_gif_fixture(), (2, 1)),
    ];
    let mut format_limits = limits();
    format_limits.max_output_dimension = 8;
    let loader = LocalImageLoader::new(format_limits);

    for (name, fixture, expected_dimensions) in cases {
        let path = dir.path().join(name);
        std::fs::write(&path, fixture).expect("write supported image fixture");
        let loaded = loader
            .load_image(
                &path,
                LocalImageRenderParams::new(/*max_width*/ 8, /*max_height*/ 8),
            )
            .await
            .expect("load supported local image");
        let prepared = image::load_from_memory_with_format(&loaded.bytes, ImageFormat::Png)
            .expect("prepared image must be PNG");

        assert!(loaded.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(!loaded.can_use_source_file);
        assert_eq!(prepared.dimensions(), expected_dimensions);
        if name.ends_with(".gif") {
            assert_eq!(prepared.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
        }
    }
}

#[tokio::test]
async fn local_image_cache_key_includes_render_params() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("diagram.png");
    std::fs::write(&path, png_fixture(/*width*/ 4, /*height*/ 2)).expect("write PNG fixture");
    let mut test_limits = limits();
    test_limits.max_cache_entries = 4;
    let loader = LocalImageLoader::new(test_limits);
    let wide_params = LocalImageRenderParams::new(/*max_width*/ 2, /*max_height*/ 2);
    let small_params = LocalImageRenderParams::new(/*max_width*/ 1, /*max_height*/ 1);

    let wide = loader
        .load_image(&path, wide_params)
        .await
        .expect("load wide prepared image");
    let small = loader
        .load_image(&path, small_params)
        .await
        .expect("load small prepared image");
    let cached_wide = loader
        .load_image(&path, wide_params)
        .await
        .expect("reuse wide prepared image");

    assert_eq!((wide.width, wide.height), (2, 1));
    assert_eq!((small.width, small.height), (1, 1));
    assert!(!Arc::ptr_eq(&wide.bytes, &small.bytes));
    assert!(Arc::ptr_eq(&wide.bytes, &cached_wide.bytes));
}
