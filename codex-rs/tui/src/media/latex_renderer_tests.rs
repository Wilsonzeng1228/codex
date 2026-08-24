use std::sync::Arc;

use super::LatexRenderError;
use super::LatexRenderRequest;
use super::LatexRenderer;

#[tokio::test]
async fn renders_formula_to_transparent_png_and_reuses_bounded_cache() {
    let renderer = LatexRenderer::default();
    let request = LatexRenderRequest::new(
        "\\frac{1}{s^2 + 2\\zeta\\omega_n s + \\omega_n^2}",
        /*display*/ true,
        /*width_cells*/ 48,
        (240, 240, 240),
    );

    let first = renderer
        .render(request.clone())
        .await
        .expect("render common control-system formula");
    assert!(first.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(first.width > 1 && first.height > 1);
    assert!(!first.can_use_source_file);
    let decoded = image::load_from_memory(&first.bytes)
        .expect("decode rendered formula")
        .to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0[3], 0);

    let cached = renderer
        .render(request.clone())
        .await
        .expect("reuse cached formula");
    assert!(Arc::ptr_eq(&first.bytes, &cached.bytes));
    assert_eq!(renderer.cache_len_for_test(), 1);

    let different_width = LatexRenderRequest::new(
        request.source,
        request.display,
        request.width_cells + 1,
        request.foreground,
    );
    renderer
        .render(different_width)
        .await
        .expect("width participates in cache key");
    assert_eq!(renderer.cache_len_for_test(), 2);

    let different_theme = LatexRenderRequest::new(
        "\\frac{1}{s^2 + 2\\zeta\\omega_n s + \\omega_n^2}",
        /*display*/ true,
        /*width_cells*/ 48,
        (16, 16, 16),
    );
    renderer
        .render(different_theme)
        .await
        .expect("terminal foreground participates in cache key");
    assert_eq!(renderer.cache_len_for_test(), 3);
}

#[tokio::test]
async fn renders_common_engineering_formula_samples() {
    let renderer = LatexRenderer::default();
    let samples = [
        "G(s)=\\frac{\\omega_n^2}{s^2+2\\zeta\\omega_n s+\\omega_n^2}",
        "X(e^{j\\omega})=\\sum_{n=-\\infty}^{\\infty}x[n]e^{-j\\omega n}",
        "\\begin{bmatrix}a&b\\\\c&d\\end{bmatrix}",
        "f(x)=\\begin{cases}x^2,&x\\ge0\\\\-x,&x<0\\end{cases}",
        "\\int_{-\\infty}^{\\infty}x(t)e^{-j2\\pi ft}\\,dt",
        "\\text{闭环传递函数 }T(s)=\\frac{G(s)}{1+G(s)H(s)}",
    ];

    for source in samples {
        let image = renderer
            .render(LatexRenderRequest::new(
                source,
                /*display*/ true,
                /*width_cells*/ 72,
                (240, 240, 240),
            ))
            .await
            .unwrap_or_else(|error| panic!("render engineering sample {source:?}: {error}"));
        assert!(image.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}

#[tokio::test]
async fn rejects_oversized_source_before_rendering() {
    let renderer = LatexRenderer::default();
    let request = LatexRenderRequest::new(
        "x".repeat(4 * 1024 + 1),
        /*display*/ false,
        /*width_cells*/ 12,
        (0, 0, 0),
    );

    assert!(matches!(
        renderer.render(request).await,
        Err(LatexRenderError::SourceTooLarge { .. })
    ));
    assert_eq!(renderer.cache_len_for_test(), 0);
}

#[tokio::test]
async fn rejects_file_inclusion_command() {
    let renderer = LatexRenderer::default();
    let request = LatexRenderRequest::new(
        "\\includegraphics{C:/Windows/win.ini}",
        /*display*/ true,
        /*width_cells*/ 30,
        (240, 240, 240),
    );

    assert!(matches!(
        renderer.render(request).await,
        Err(LatexRenderError::Parse { .. })
    ));
}
