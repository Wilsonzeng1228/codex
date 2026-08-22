use pretty_assertions::assert_eq;
use ratatui::layout::Rect;

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

#[test]
fn active_redraw_retires_old_placement_and_rebuilds_resized_rect() {
    let mut registry = MediaPlacementRegistry::default();
    let first = registry.replace_active(vec![request(
        "D:/course/diagram.png",
        Rect::new(
            /*x*/ 2, /*y*/ 4, /*width*/ 20, /*height*/ 3,
        ),
    )]);
    let first_id = first.added[0].id;

    let resized = registry.replace_active(vec![request(
        "D:/course/diagram.png",
        Rect::new(
            /*x*/ 2, /*y*/ 5, /*width*/ 30, /*height*/ 3,
        ),
    )]);

    assert_eq!(resized.retired, vec![first_id]);
    assert_eq!(resized.added.len(), 1);
    assert_ne!(resized.added[0].id, first_id);
    assert_eq!(
        resized.added[0].request.rect,
        Rect::new(
            /*x*/ 2, /*y*/ 5, /*width*/ 30, /*height*/ 3
        )
    );
    assert_eq!(registry.active(), resized.added.as_slice());
}

#[test]
fn active_redraw_preserves_history_until_history_is_explicitly_rebuilt() {
    let mut registry = MediaPlacementRegistry::default();
    let history = registry.append_history(vec![request(
        "D:/course/history.png",
        Rect::new(
            /*x*/ 2, /*y*/ 8, /*width*/ 24, /*height*/ 3,
        ),
    )]);
    let history_id = history.added[0].id;

    registry.replace_active(vec![request(
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
