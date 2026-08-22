use std::fs;

use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use serial_test::serial;

use super::AnchoredMediaPlacementRequest;
use super::ImageProtocol;
use super::MediaCellId;
use super::MediaNode;
use super::MediaPlacementRegistry;
use super::MediaPlacementRequest;
use super::write_kitty_placement_update;

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

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

#[test]
#[serial]
fn kitty_writer_deletes_retired_ids_before_replaying_local_png_placement() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let first_path = dir.path().join("first.png");
    let second_path = dir.path().join("second.png");
    fs::write(&first_path, [PNG_SIGNATURE.as_slice(), b"one"].concat())
        .expect("write first png fixture");
    fs::write(&second_path, [PNG_SIGNATURE.as_slice(), b"two"].concat())
        .expect("write second png fixture");
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

    let report = write_kitty_placement_update(&mut output, ImageProtocol::Kitty, &update)
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
fn kitty_writer_skips_remote_sources_without_emitting_protocol_bytes_for_them() {
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

    let report = write_kitty_placement_update(&mut output, ImageProtocol::Kitty, &update)
        .expect("skip unsupported remote placement");

    assert!(output.is_empty());
    assert_eq!(report.placed, 0);
    assert_eq!(report.skipped, 1);
}

#[test]
#[serial]
fn kitty_writer_skips_local_files_without_a_png_signature() {
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

    let report = write_kitty_placement_update(&mut output, ImageProtocol::Kitty, &update)
        .expect("skip local non-PNG placement");

    assert!(output.is_empty());
    assert_eq!(report.placed, 0);
    assert_eq!(report.skipped, 1);
}
