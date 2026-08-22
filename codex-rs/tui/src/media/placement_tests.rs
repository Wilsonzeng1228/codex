use pretty_assertions::assert_eq;
use ratatui::layout::Rect;

use super::AnchoredMediaPlacementRequest;
use super::MediaCellId;
use super::MediaNode;
use super::MediaPlacementRegistry;
use super::MediaPlacementRequest;

fn request(source: &str, rect: Rect) -> MediaPlacementRequest {
    MediaPlacementRequest {
        node: MediaNode::Image {
            source: source.to_string(),
            alt: "diagram".to_string(),
            ordinal: 0,
        },
        rect,
    }
}

fn anchored_request(
    cell_id: MediaCellId,
    source: &str,
    rect: Rect,
) -> AnchoredMediaPlacementRequest {
    AnchoredMediaPlacementRequest::new(cell_id, request(source, rect))
}

#[test]
fn active_redraw_preserves_id_and_rebuilds_resized_rect() {
    let mut registry = MediaPlacementRegistry::default();
    let cell_id = MediaCellId::new(7).expect("non-zero media cell id");
    let first = registry.replace_active(vec![anchored_request(
        cell_id,
        "D:/course/diagram.png",
        Rect::new(
            /*x*/ 2, /*y*/ 4, /*width*/ 20, /*height*/ 3,
        ),
    )]);
    let first_id = first.placed[0].id;

    let resized = registry.replace_active(vec![anchored_request(
        cell_id,
        "D:/course/diagram.png",
        Rect::new(
            /*x*/ 2, /*y*/ 5, /*width*/ 30, /*height*/ 3,
        ),
    )]);

    assert_eq!(resized.retired, Vec::new());
    assert_eq!(resized.placed.len(), 1);
    assert_eq!(resized.placed[0].id, first_id);
    assert_eq!(
        resized.placed[0].request.request.rect,
        Rect::new(
            /*x*/ 2, /*y*/ 5, /*width*/ 30, /*height*/ 3
        )
    );
    assert_eq!(registry.active(), resized.placed.as_slice());
}

#[test]
fn active_redraw_preserves_history_until_history_is_explicitly_rebuilt() {
    let mut registry = MediaPlacementRegistry::default();
    let history_cell = MediaCellId::new(8).expect("non-zero media cell id");
    let active_cell = MediaCellId::new(9).expect("non-zero media cell id");
    let history = registry.append_history(vec![anchored_request(
        history_cell,
        "D:/course/history.png",
        Rect::new(
            /*x*/ 2, /*y*/ 8, /*width*/ 24, /*height*/ 3,
        ),
    )]);
    let history_id = history.placed[0].id;

    registry.replace_active(vec![anchored_request(
        active_cell,
        "D:/course/active.png",
        Rect::new(
            /*x*/ 2, /*y*/ 1, /*width*/ 24, /*height*/ 3,
        ),
    )]);
    let cleared = registry.replace_history(Vec::new());

    assert_eq!(cleared.retired, vec![history_id]);
    assert_eq!(registry.history(), &[]);
    assert_eq!(registry.active().len(), 1);
}

#[test]
fn committing_the_same_anchor_promotes_active_placement_without_allocating_a_new_id() {
    let mut registry = MediaPlacementRegistry::default();
    let cell_id = MediaCellId::new(10).expect("non-zero media cell id");
    let active = registry.replace_active(vec![anchored_request(
        cell_id,
        "D:/course/diagram.png",
        Rect::new(
            /*x*/ 2, /*y*/ 4, /*width*/ 20, /*height*/ 3,
        ),
    )]);
    let active_id = active.placed[0].id;

    let committed = registry.append_history(vec![anchored_request(
        cell_id,
        "D:/course/diagram.png",
        Rect::new(
            /*x*/ 2, /*y*/ 12, /*width*/ 20, /*height*/ 3,
        ),
    )]);

    assert_eq!(committed.retired, Vec::new());
    assert_eq!(committed.placed[0].id, active_id);
    assert!(registry.active().is_empty());
    assert_eq!(registry.history()[0].id, active_id);
}

#[test]
fn scoped_history_reflow_preserves_older_scrollback_placements() {
    let mut registry = MediaPlacementRegistry::default();
    let older_cell = MediaCellId::new(11).expect("non-zero media cell id");
    let rebuilt_cell = MediaCellId::new(12).expect("non-zero media cell id");
    let initial = registry.append_history(vec![
        anchored_request(
            older_cell,
            "D:/course/older.png",
            Rect::new(
                /*x*/ 2, /*y*/ 2, /*width*/ 20, /*height*/ 3,
            ),
        ),
        anchored_request(
            rebuilt_cell,
            "D:/course/rebuilt.png",
            Rect::new(
                /*x*/ 2, /*y*/ 8, /*width*/ 20, /*height*/ 3,
            ),
        ),
    ]);
    let older_id = initial.placed[0].id;
    let rebuilt_id = initial.placed[1].id;

    let reflow = registry.replace_history_scope(
        &[rebuilt_cell],
        vec![anchored_request(
            rebuilt_cell,
            "D:/course/rebuilt.png",
            Rect::new(
                /*x*/ 2, /*y*/ 6, /*width*/ 30, /*height*/ 3,
            ),
        )],
    );

    assert_eq!(reflow.retired, Vec::new());
    assert_eq!(reflow.placed[0].id, rebuilt_id);
    assert_eq!(registry.history().len(), 2);
    assert_eq!(registry.history()[0].id, older_id);
    assert_eq!(registry.history()[1].id, rebuilt_id);

    let removed = registry.replace_history_scope(&[rebuilt_cell], Vec::new());
    assert_eq!(removed.retired, vec![rebuilt_id]);
    assert_eq!(registry.history().len(), 1);
    assert_eq!(registry.history()[0].id, older_id);
}
