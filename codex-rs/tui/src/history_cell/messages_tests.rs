use super::*;
use crate::history_cell::markdown_render_cache::MarkdownRenderCacheKey;
use assert_matches::assert_matches;
use pretty_assertions::assert_eq;

#[test]
fn finalized_markdown_exposes_image_media_nodes_without_changing_raw_source() {
    let source = "Before ![system *diagram*](D:/course/diagram.png) after.";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));

    assert_eq!(
        cell.media_nodes(),
        &[crate::media::MediaNode::Image {
            source: "D:/course/diagram.png".to_string(),
            alt: "system diagram".to_string(),
            ordinal: 0,
        }]
    );
    assert_eq!(cell.raw_lines(), vec![Line::from(source)]);
}

#[test]
fn finalized_markdown_media_layout_reserves_rows_at_each_image_ordinal() {
    let source = concat!(
        "Before\n\n",
        "![first](https://example.org/first.png)\n\n",
        "Between\n\n",
        "![second](file:///D:/course/second.png)\n\n",
        "After",
    );
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));
    let placeholder_rows =
        crate::media::MediaPlaceholderRows::try_from(3).expect("non-zero placeholder height");

    let layout = cell.display_media_layout(/*width*/ 32, Some(placeholder_rows));

    assert_eq!(
        layout.placements,
        vec![
            crate::media::MediaPlacementRequest {
                node: crate::media::MediaNode::Image {
                    source: "https://example.org/first.png".to_string(),
                    alt: "first".to_string(),
                    ordinal: 0,
                },
                rect: Rect::new(
                    /*x*/ 2, /*y*/ 2, /*width*/ 30, /*height*/ 3
                ),
            },
            crate::media::MediaPlacementRequest {
                node: crate::media::MediaNode::Image {
                    source: "file:///D:/course/second.png".to_string(),
                    alt: "second".to_string(),
                    ordinal: 1,
                },
                rect: Rect::new(
                    /*x*/ 2, /*y*/ 8, /*width*/ 30, /*height*/ 3
                ),
            },
        ]
    );
    assert_eq!(layout.lines.len(), 13);
    assert_eq!(
        cell.raw_lines(),
        vec![
            Line::from("Before"),
            Line::default(),
            Line::from("![first](https://example.org/first.png)"),
            Line::default(),
            Line::from("Between"),
            Line::default(),
            Line::from("![second](file:///D:/course/second.png)"),
            Line::default(),
            Line::from("After"),
        ]
    );
    assert!(
        layout
            .lines
            .iter()
            .flat_map(|line| &line.line.spans)
            .all(|span| !span.content.contains('\x1b'))
    );

    let fallback_layout =
        cell.display_media_layout(/*width*/ 32, /*image_placeholder_rows*/ None);
    let fallback_text = visible_lines(fallback_layout.lines)
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(fallback_layout.placements.is_empty());
    assert!(fallback_text.contains("[image: first]"));
    assert!(fallback_text.contains("[image: second]"));
}

#[test]
fn finalized_markdown_media_layout_keeps_block_latex_fallback_under_placement() {
    let source = "Before\n\n$$\\frac{1}{s+1}$$\n\nAfter";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));
    let placeholder_rows =
        crate::media::MediaPlaceholderRows::try_from(3).expect("non-zero placeholder height");

    let layout = cell.display_media_layout(/*width*/ 32, Some(placeholder_rows));
    let visible_text = visible_lines(layout.lines.clone())
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        layout.placements,
        vec![crate::media::MediaPlacementRequest {
            node: crate::media::MediaNode::Latex {
                source: "\\frac{1}{s+1}".to_string(),
                display: true,
                ordinal: 0,
            },
            rect: Rect::new(
                /*x*/ 2, /*y*/ 2, /*width*/ 30, /*height*/ 3
            ),
        }]
    );
    assert!(visible_text.contains("$$\\frac{1}{s+1}$$"));
    assert_eq!(cell.raw_lines(), raw_lines_from_source(source));
}

#[test]
fn finalized_markdown_media_layout_reserves_two_rows_for_readable_inline_latex() {
    let source = "Gain is $x^2+y^2$";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));
    let placeholder_rows =
        crate::media::MediaPlaceholderRows::try_from(3).expect("non-zero placeholder height");

    let layout = cell.display_media_layout(/*width*/ 64, Some(placeholder_rows));
    let visible = visible_lines(layout.lines.clone());

    assert_eq!(visible.len(), 2);
    assert_eq!(visible[0].to_string(), "• Gain is $x^2+y^2$");
    assert!(visible[1].to_string().trim().is_empty());
    assert_eq!(
        layout.placements,
        vec![crate::media::MediaPlacementRequest {
            node: crate::media::MediaNode::Latex {
                source: "x^2+y^2".to_string(),
                display: false,
                ordinal: 0,
            },
            rect: Rect::new(
                /*x*/ 10, /*y*/ 0, /*width*/ 9, /*height*/ 2
            ),
        }]
    );
    insta::assert_snapshot!(
        format!(
            "visible:\n{}\n\nplacements:\n{:#?}",
            visible.iter().map(Line::to_string).collect::<Vec<_>>().join("\n"),
            layout.placements
        ),
        @r###"
    visible:
    • Gain is $x^2+y^2$


    placements:
    [
        MediaPlacementRequest {
            node: Latex {
                source: "x^2+y^2",
                display: false,
                ordinal: 0,
            },
            rect: Rect {
                x: 10,
                y: 0,
                width: 9,
                height: 2,
            },
        },
    ]
    "###
    );
}

#[test]
fn finalized_markdown_without_media_capability_keeps_raw_latex_only() {
    let source = "Inline $x^2$ and block:\n\n$$\\int_0^1 x\\,dx$$";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));

    let layout = cell.display_media_layout(/*width*/ 48, /*image_placeholder_rows*/ None);
    let visible_text = visible_lines(layout.lines)
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(layout.placements.is_empty());
    assert!(visible_text.contains("$x^2$"));
    assert!(visible_text.contains("$$\\int_0^1 x\\,dx$$"));
    assert_eq!(cell.raw_lines(), raw_lines_from_source(source));
}

#[test]
fn invalid_image_source_keeps_text_fallback_in_media_layout() {
    let source = "![internal](http://127.0.0.1/private.png)";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));
    let placeholder_rows =
        crate::media::MediaPlaceholderRows::try_from(3).expect("non-zero placeholder height");

    let layout = cell.display_media_layout(/*width*/ 48, Some(placeholder_rows));
    let visible = visible_lines(layout.lines);
    let visible_text = visible
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_visible_text = visible_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert!(layout.placements.is_empty());
    assert!(normalized_visible_text.contains("[image: internal]"));
    assert!(
        normalized_visible_text.contains("[image unavailable:"),
        "unexpected fallback text: {visible_text:?}"
    );
}

#[test]
fn sanitizer_borrows_clean_text_and_removes_control_sequences() {
    for (text, expected) in [
        ("clean\ttext\n", "clean\ttext\n"),
        ("\x07before", "before"),
        ("before\x07", "before"),
        ("\x1b[31mbefore", "before"),
        ("before\x1b[31m", "before"),
        ("before\x1b[31", "before"),
        ("\x07[31m", "[31m"),
        ("\x07", ""),
    ] {
        assert_matches!(
            sanitize_user_text(text.into()),
            Cow::Borrowed(sanitized) => assert_eq!(sanitized, expected)
        );
    }
    assert_matches!(
        sanitize_user_text("before\x1b[31mafter\x07".into()),
        Cow::Owned(sanitized) => assert_eq!(sanitized, "beforeafter")
    );
    assert_eq!(sanitize_user_text("é\u{85}中".into()), "é中");
    assert_eq!(sanitize_user_text("before\x1bafter".into()), "beforeafter");
}

#[test]
fn sanitizer_preserves_owned_buffer_for_clean_and_edge_trimmed_text() {
    for (text, expected) in [
        ("clean\ttext\n", "clean\ttext\n"),
        ("\x07before", "before"),
        ("before\x07", "before"),
        ("\x07before\x07", "before"),
        ("\x1b[31mbefore", "before"),
        ("before\x1b[31m", "before"),
        ("\x07", ""),
    ] {
        let owned = text.to_string();
        let original_pointer = owned.as_ptr();
        let original_capacity = owned.capacity();

        assert_matches!(sanitize_user_text(owned.into()), Cow::Owned(sanitized) => {
            assert_eq!(sanitized, expected);
            assert_eq!(sanitized.as_ptr(), original_pointer);
            assert_eq!(sanitized.capacity(), original_capacity);
        })
    }
}

#[test]
fn sanitizer_preallocates_owned_multi_fragment_text() {
    let text = "before\x1b[31mafter\x07".to_string();
    let original_length = text.len();

    assert_matches!(sanitize_user_text(text.into()), Cow::Owned(sanitized) => {
        assert_eq!(sanitized, "beforeafter");
        assert!(sanitized.capacity() >= original_length, "{} >= {}", sanitized.capacity(), original_length);
    })
}

fn replace_cached_lines(
    cell: &AgentMarkdownCell,
    update_key: impl FnOnce(&mut MarkdownRenderCacheKey),
) {
    let rendered_lines = cell
        .rendered_lines
        .as_ref()
        .expect("ordinary markdown should be cacheable");
    let mut rendered_lines = rendered_lines.cached.lock().expect("render cache lock");
    let (key, lines) = rendered_lines
        .as_mut()
        .expect("render cache should be populated");
    *lines = vec![HyperlinkLine::from("cached")];
    update_key(key);
}

#[test]
fn finalized_markdown_reuses_lines_primed_by_transcript_height() {
    let cell = AgentMarkdownCell::new("finalized **markdown**".to_string(), Path::new("/tmp"));
    let width = 48;

    assert_eq!(cell.desired_transcript_height(width), 1);
    replace_cached_lines(&cell, |_| {});

    assert_eq!(
        visible_lines(cell.transcript_hyperlink_lines(width)),
        vec![Line::from("cached")]
    );
}

#[test]
fn finalized_markdown_cache_misses_when_width_or_render_style_changes() {
    let cell = AgentMarkdownCell::new("finalized **markdown**".to_string(), Path::new("/tmp"));
    let width = 48;
    let expected = cell.display_lines(width);

    replace_cached_lines(&cell, |key| key.width = key.width.saturating_sub(1));
    assert_eq!(cell.display_lines(width), expected);

    replace_cached_lines(&cell, |key| {
        key.syntax_theme_revision = key.syntax_theme_revision.wrapping_sub(1);
    });
    assert_eq!(cell.display_lines(width), expected);

    replace_cached_lines(&cell, |key| {
        key.terminal_fg = key
            .terminal_fg
            .map_or(Some((1, 2, 3)), |(r, g, b)| Some((r ^ 1, g, b)));
    });
    assert_eq!(cell.display_lines(width), expected);
}

#[test]
fn raw_markdown_bypasses_the_rich_render_cache() {
    let source = "finalized **markdown**";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));
    let width = 48;

    cell.display_lines(width);
    replace_cached_lines(&cell, |_| {});

    assert_eq!(
        cell.display_lines_for_mode(width, HistoryRenderMode::Raw),
        vec![Line::from(source)]
    );
}

#[test]
fn visualization_directives_are_not_cached() {
    for markdown in [
        "::codex-inline-vis{file=\"chart.html\"}",
        "\u{e200}visualize\u{e202}{\"path\":\"/tmp/chart.html\"}\u{e201}",
    ] {
        let cell = AgentMarkdownCell::new(markdown.to_string(), Path::new("/tmp"));

        cell.display_lines(/*width*/ 48);

        assert!(cell.rendered_lines.is_none());
    }
}
