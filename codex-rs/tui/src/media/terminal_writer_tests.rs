use std::fs;
use std::io::Cursor;
use std::sync::Arc;

use image::DynamicImage;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use serial_test::serial;

use super::AnchoredMediaPlacementRequest;
use super::ImageProtocol;
use super::MediaCellId;
use super::MediaImageState;
use super::MediaNode;
use super::MediaPlacementRegistry;
use super::MediaPlacementRequest;
use super::local_loader::LoadedLocalImage;
use super::write_media_placement_update;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

fn png_fixture() -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::new_rgba8(/*width*/ 1, /*height*/ 1)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("encode PNG fixture");
    encoded.into_inner()
}

fn loaded_fixture(bytes: Vec<u8>) -> LoadedLocalImage {
    LoadedLocalImage {
        bytes: Arc::from(bytes),
        source_width: 1,
        source_height: 1,
        width: 1,
        height: 1,
        can_use_source_file: true,
    }
}

fn local_request(
    cell_id: MediaCellId,
    source: String,
    rect: Rect,
) -> AnchoredMediaPlacementRequest {
    AnchoredMediaPlacementRequest::new(
        cell_id,
        MediaPlacementRequest {
            node: MediaNode::Image {
                source,
                alt: "diagram".to_string(),
                ordinal: 0,
            },
            rect,
        },
    )
}

fn latex_request(cell_id: MediaCellId, source: &str, rect: Rect) -> AnchoredMediaPlacementRequest {
    AnchoredMediaPlacementRequest::new(
        cell_id,
        MediaPlacementRequest {
            node: MediaNode::Latex {
                source: source.to_string(),
                display: true,
                ordinal: 0,
            },
            rect,
        },
    )
}

#[test]
#[serial]
fn kitty_writer_deletes_retired_ids_before_replaying_local_png_placement() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let first_path = dir.path().join("first.png");
    let second_path = dir.path().join("second.png");
    fs::write(&first_path, png_fixture()).expect("write first png fixture");
    fs::write(&second_path, png_fixture()).expect("write second png fixture");
    let first_cell = MediaCellId::new(21).expect("non-zero media cell id");
    let second_cell = MediaCellId::new(22).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    registry.replace_active(vec![local_request(
        first_cell,
        first_path.to_string_lossy().into_owned(),
        Rect::new(
            /*x*/ 1, /*y*/ 2, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let update = registry.replace_active(vec![local_request(
        second_cell,
        second_path.to_string_lossy().into_owned(),
        Rect::new(
            /*x*/ 4, /*y*/ 5, /*width*/ 12, /*height*/ 4,
        ),
    )]);
    let retired_id = update.retired[0];
    let placed_id = update.placed[0].id;
    let request_before_write = update.placed[0].request.clone();
    let mut output = Vec::new();
    let loaded = loaded_fixture(png_fixture());

    let report = write_media_placement_update(&mut output, ImageProtocol::Kitty, &update, |_| {
        MediaImageState::Ready(loaded.clone())
    })
    .expect("write placement update");
    let output = String::from_utf8(output).expect("Kitty commands are UTF-8");
    let delete_offset = output
        .find(&format!("a=d,d=I,i={},q=2", retired_id.get()))
        .expect("retired image deletion command");
    let transmit_offset = output
        .find(&format!("a=T,t=d,f=100,c=12,r=4,q=2,i={}", placed_id.get()))
        .expect("replacement image transmission command");

    assert!(delete_offset < transmit_offset);
    assert!(
        output.contains("\x1b[6;5H"),
        "placement must move to its Ratatui rect"
    );
    assert_eq!(report.placed, 1);
    assert_eq!(report.skipped, 0);
    assert_eq!(update.placed[0].request, request_before_write);
}

#[test]
#[serial]
fn kitty_writer_skips_unavailable_remote_source() {
    let cell_id = MediaCellId::new(23).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        cell_id,
        "https://example.com/diagram.png".to_string(),
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let mut output = Vec::new();

    let report = write_media_placement_update(&mut output, ImageProtocol::Kitty, &update, |_| {
        MediaImageState::Unavailable
    })
    .expect("skip unsupported remote placement");

    assert!(output.is_empty());
    assert_eq!(report.placed, 0);
    assert_eq!(report.skipped, 1);
}

#[test]
#[serial]
fn iterm2_writer_transmits_ready_https_image_bytes() {
    let cell_id = MediaCellId::new(29).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        cell_id,
        "https://example.com/diagram.png".to_string(),
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let fixture = png_fixture();
    let loaded = loaded_fixture(fixture.clone());
    let mut output = Vec::new();

    let report =
        write_media_placement_update(&mut output, ImageProtocol::Iterm2Inline, &update, |_| {
            MediaImageState::Ready(loaded.clone())
        })
        .expect("write downloaded HTTPS image bytes");
    let output = String::from_utf8(output).expect("iTerm2 command is UTF-8");

    assert!(output.contains(&format!(
        "\x1b]1337;File=size={};width=10;height=3;inline=1:",
        fixture.len()
    )));
    assert_eq!(report.placed, 1);
    assert_eq!(report.skipped, 0);
}

#[test]
#[serial]
fn iterm2_writer_transmits_ready_latex_png_bytes() {
    let cell_id = MediaCellId::new(32).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![latex_request(
        cell_id,
        "\\frac{1}{s+1}",
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 30, /*height*/ 3,
        ),
    )]);
    let fixture = png_fixture();
    let loaded = loaded_fixture(fixture.clone());
    let mut output = Vec::new();

    let report =
        write_media_placement_update(&mut output, ImageProtocol::Iterm2Inline, &update, |_| {
            MediaImageState::Ready(loaded.clone())
        })
        .expect("write rendered LaTeX PNG bytes");
    let output = String::from_utf8(output).expect("iTerm2 command is UTF-8");

    assert!(output.contains(&format!(
        "\x1b]1337;File=size={};width=30;height=3;inline=1:",
        fixture.len()
    )));
    assert!(
        output.contains(&format!("\x1b[4;3H{}", " ".repeat(30))),
        "ready transparent LaTeX must clear the fallback cells before transmission"
    );
    assert_eq!(report.placed, 1);
    assert_eq!(report.skipped, 0);
}

#[test]
#[serial]
fn kitty_local_file_writer_sends_ready_https_image_as_data() {
    let cell_id = MediaCellId::new(30).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        cell_id,
        "https://example.com/diagram.png".to_string(),
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let loaded = loaded_fixture(png_fixture());
    let mut output = Vec::new();

    let report =
        write_media_placement_update(&mut output, ImageProtocol::KittyLocalFile, &update, |_| {
            MediaImageState::Ready(loaded.clone())
        })
        .expect("write downloaded HTTPS image bytes");
    let output = String::from_utf8(output).expect("Kitty command is UTF-8");

    assert!(output.contains("a=T,t=d,f=100,c=10,r=3,q=2"));
    assert!(!output.contains("a=T,t=f,f=100"));
    assert_eq!(report.placed, 1);
    assert_eq!(report.skipped, 0);
}

#[test]
#[serial]
fn kitty_writer_skips_unavailable_local_image() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("not-really.png");
    fs::write(&path, b"plain text").expect("write non-PNG fixture");
    let cell_id = MediaCellId::new(24).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        cell_id,
        path.to_string_lossy().into_owned(),
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let mut output = Vec::new();

    let report = write_media_placement_update(&mut output, ImageProtocol::Kitty, &update, |_| {
        MediaImageState::Unavailable
    })
    .expect("skip local non-PNG placement");

    assert!(output.is_empty());
    assert_eq!(report.placed, 0);
    assert_eq!(report.skipped, 1);
}

#[test]
#[serial]
fn kitty_writer_skips_locally_malformed_png_after_signature() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("truncated.png");
    fs::write(
        &path,
        [PNG_SIGNATURE.as_slice(), b"not-a-png-body"].concat(),
    )
    .expect("write malformed PNG fixture");
    let cell_id = MediaCellId::new(27).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        cell_id,
        path.to_string_lossy().into_owned(),
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let mut output = Vec::new();

    let report = write_media_placement_update(&mut output, ImageProtocol::Kitty, &update, |_| {
        MediaImageState::Unavailable
    })
    .expect("skip malformed local PNG placement");

    assert!(output.is_empty());
    assert_eq!(report.placed, 0);
    assert_eq!(report.skipped, 1);
}

#[test]
#[serial]
fn iterm2_writer_restores_cursor_without_suppressing_protocol_cursor_movement() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let first_path = dir.path().join("first.png");
    let second_path = dir.path().join("second.png");
    fs::write(&first_path, png_fixture()).expect("write first png fixture");
    let second_fixture = png_fixture();
    fs::write(&second_path, &second_fixture).expect("write second png fixture");
    let first_cell = MediaCellId::new(25).expect("non-zero media cell id");
    let second_cell = MediaCellId::new(26).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    registry.replace_active(vec![local_request(
        first_cell,
        first_path.to_string_lossy().into_owned(),
        Rect::new(1, 2, 10, 3),
    )]);
    let update = registry.replace_active(vec![local_request(
        second_cell,
        second_path.to_string_lossy().into_owned(),
        Rect::new(4, 5, 12, 4),
    )]);
    let mut output = Vec::new();

    let loaded = loaded_fixture(second_fixture.clone());
    let report =
        write_media_placement_update(&mut output, ImageProtocol::Iterm2Inline, &update, |_| {
            MediaImageState::Ready(loaded.clone())
        })
        .expect("write iTerm2 placement update");
    let output = String::from_utf8(output).expect("iTerm2 command is UTF-8");

    assert!(output.contains(&format!(
        "\x1b]1337;File=size={};width=12;height=4;inline=1:",
        second_fixture.len()
    )));
    assert!(!output.contains("doNotMoveCursor"));
    assert!(output.contains("\x1b\\\x1b8"));
    assert!(!output.contains("\x1b_G"));
    assert!(output.contains("\x1b[6;5H"));
    assert_eq!(report.placed, 1);
    assert_eq!(report.skipped, 0);
}

#[test]
#[serial]
fn kitty_local_file_writer_uses_prepared_png_bytes_when_source_cannot_be_reused() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("diagram.jpg");
    fs::write(&path, b"source bytes are not used by the injected writer")
        .expect("write source fixture");
    let cell_id = MediaCellId::new(28).expect("non-zero media cell id");
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        cell_id,
        path.to_string_lossy().into_owned(),
        Rect::new(
            /*x*/ 2, /*y*/ 3, /*width*/ 10, /*height*/ 3,
        ),
    )]);
    let mut output = Vec::new();
    let mut loaded = loaded_fixture(png_fixture());
    loaded.can_use_source_file = false;

    let report =
        write_media_placement_update(&mut output, ImageProtocol::KittyLocalFile, &update, |_| {
            MediaImageState::Ready(loaded.clone())
        })
        .expect("write prepared PNG bytes for non-reusable source");
    let output = String::from_utf8(output).expect("Kitty commands are UTF-8");

    assert!(output.contains("a=T,t=d,f=100,c=10,r=3,q=2"));
    assert!(!output.contains("a=T,t=f,f=100"));
    assert_eq!(report.placed, 1);
}
