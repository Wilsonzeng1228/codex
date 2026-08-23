use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use tokio::sync::Notify;
use tokio::time::timeout;

use super::AnchoredMediaPlacementRequest;
use super::MediaCellId;
use super::MediaNode;
use super::MediaPlacementRegistry;
use super::MediaPlacementRequest;
use super::load_coordinator::LocalImageLoadFuture;
use super::load_coordinator::MediaLoadCompletion;
use super::load_coordinator::MediaLoadCoordinator;
use super::load_coordinator::MediaPlacementDomain;
use super::local_loader::LoadedLocalImage;
use crate::tui::FrameRequester;

fn loaded_image() -> LoadedLocalImage {
    LoadedLocalImage {
        bytes: Arc::from(&b"prepared-png"[..]),
        source_width: 1,
        source_height: 1,
        width: 1,
        height: 1,
    }
}

fn local_request(cell_id: u64, ordinal: usize, path: PathBuf) -> AnchoredMediaPlacementRequest {
    AnchoredMediaPlacementRequest::new(
        MediaCellId::new(cell_id).expect("non-zero cell id"),
        MediaPlacementRequest {
            node: MediaNode::Image {
                source: path.to_string_lossy().into_owned(),
                alt: "diagram".to_string(),
                ordinal,
            },
            rect: Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 4,
            ),
        },
    )
}

#[tokio::test]
async fn duplicate_paths_share_one_in_flight_load() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("shared.png");
    let starts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let loader = {
        let starts = Arc::clone(&starts);
        let release = Arc::clone(&release);
        Arc::new(move |_path: PathBuf| -> LocalImageLoadFuture {
            let starts = Arc::clone(&starts);
            let release = Arc::clone(&release);
            Box::pin(async move {
                starts.fetch_add(1, Ordering::SeqCst);
                release.notified().await;
                Ok(loaded_image())
            })
        })
    };
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![
        local_request(/*cell_id*/ 1, /*ordinal*/ 0, path.clone()),
        local_request(/*cell_id*/ 2, /*ordinal*/ 0, path),
    ]);
    let (frame_requester, _frame_rx) = FrameRequester::test_channel();
    let mut coordinator =
        MediaLoadCoordinator::with_loader(frame_requester, /*max_concurrent*/ 2, loader);

    coordinator.reconcile(&update, MediaPlacementDomain::Active);
    tokio::task::yield_now().await;

    assert_eq!(starts.load(Ordering::SeqCst), 1);
    release.notify_waiters();
}

#[tokio::test]
async fn coordinator_limits_concurrent_loads() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let starts = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let loader = {
        let starts = Arc::clone(&starts);
        let active = Arc::clone(&active);
        let max_active = Arc::clone(&max_active);
        let release = Arc::clone(&release);
        Arc::new(move |_path: PathBuf| -> LocalImageLoadFuture {
            let starts = Arc::clone(&starts);
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            let release = Arc::clone(&release);
            Box::pin(async move {
                starts.fetch_add(1, Ordering::SeqCst);
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(current, Ordering::SeqCst);
                release.notified().await;
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(loaded_image())
            })
        })
    };
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(
        (1..=3)
            .map(|cell_id| {
                local_request(
                    cell_id,
                    /*ordinal*/ 0,
                    dir.path().join(format!("{cell_id}.png")),
                )
            })
            .collect(),
    );
    let (frame_requester, _frame_rx) = FrameRequester::test_channel();
    let mut coordinator =
        MediaLoadCoordinator::with_loader(frame_requester, /*max_concurrent*/ 2, loader);

    coordinator.reconcile(&update, MediaPlacementDomain::Active);
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }

    assert_eq!(starts.load(Ordering::SeqCst), 2);
    assert_eq!(max_active.load(Ordering::SeqCst), 2);
    release.notify_waiters();
}

#[tokio::test]
async fn completed_history_load_requests_frame_and_reflow() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("history.png");
    let loader = Arc::new(move |_path: PathBuf| -> LocalImageLoadFuture {
        Box::pin(async move { Ok(loaded_image()) })
    });
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.append_history(vec![local_request(
        /*cell_id*/ 3, /*ordinal*/ 0, path,
    )]);
    let placement = update.placed[0].clone();
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator =
        MediaLoadCoordinator::with_loader(frame_requester, /*max_concurrent*/ 2, loader);

    coordinator.reconcile(&update, MediaPlacementDomain::History);
    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("completed load should request a frame")
        .expect("frame requester should remain connected");

    assert_eq!(
        coordinator.poll_completed(),
        MediaLoadCompletion {
            active_ready: false,
            history_ready: true,
        }
    );
    assert!(coordinator.ready_image(&placement).is_some());
}

#[tokio::test]
async fn retired_placement_ignores_queued_completion() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let path = dir.path().join("stale.png");
    let loader = Arc::new(move |_path: PathBuf| -> LocalImageLoadFuture {
        Box::pin(async move { Ok(loaded_image()) })
    });
    let mut registry = MediaPlacementRegistry::default();
    let placed = registry.replace_active(vec![local_request(
        /*cell_id*/ 4, /*ordinal*/ 0, path,
    )]);
    let placement = placed.placed[0].clone();
    let retired = registry.replace_active(Vec::new());
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator =
        MediaLoadCoordinator::with_loader(frame_requester, /*max_concurrent*/ 2, loader);

    coordinator.reconcile(&placed, MediaPlacementDomain::Active);
    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("completed load should request a frame")
        .expect("frame requester should remain connected");
    coordinator.reconcile(&retired, MediaPlacementDomain::Active);

    assert_eq!(coordinator.poll_completed(), MediaLoadCompletion::default());
    assert!(coordinator.ready_image(&placement).is_none());
}
