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
use super::load_coordinator::LocalImageLoadFn;
use super::load_coordinator::LocalImageLoadFuture;
use super::load_coordinator::MediaImageState;
use super::load_coordinator::MediaLoadCompletion;
use super::load_coordinator::MediaLoadCoordinator;
use super::load_coordinator::MediaPlacementDomain;
use super::load_coordinator::RemoteImageLoadFuture;
use super::local_loader::LoadedLocalImage;
use super::remote_loader::RemoteImageDownloadError;
use crate::tui::FrameRequester;

fn loaded_image() -> LoadedLocalImage {
    LoadedLocalImage {
        bytes: Arc::from(&b"prepared-png"[..]),
        source_width: 1,
        source_height: 1,
        width: 1,
        height: 1,
        can_use_source_file: true,
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

fn https_request(cell_id: u64, ordinal: usize, source: &str) -> AnchoredMediaPlacementRequest {
    AnchoredMediaPlacementRequest::new(
        MediaCellId::new(cell_id).expect("non-zero cell id"),
        MediaPlacementRequest {
            node: MediaNode::Image {
                source: source.to_string(),
                alt: "remote diagram".to_string(),
                ordinal,
            },
            rect: Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 4,
            ),
        },
    )
}

fn latex_request(cell_id: u64, ordinal: usize, source: &str) -> AnchoredMediaPlacementRequest {
    AnchoredMediaPlacementRequest::new(
        MediaCellId::new(cell_id).expect("non-zero cell id"),
        MediaPlacementRequest {
            node: MediaNode::Latex {
                source: source.to_string(),
                display: true,
                ordinal,
            },
            rect: Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 3,
            ),
        },
    )
}

fn unused_local_loader() -> Arc<LocalImageLoadFn> {
    Arc::new(move |_path: PathBuf| -> LocalImageLoadFuture {
        Box::pin(async move { panic!("remote-only test must not start the local loader") })
    })
}

#[tokio::test]
async fn clear_cache_restarts_live_sources() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let starts = Arc::new(AtomicUsize::new(0));
    let loader = {
        let starts = Arc::clone(&starts);
        Arc::new(move |_path: PathBuf| -> LocalImageLoadFuture {
            let starts = Arc::clone(&starts);
            Box::pin(async move {
                starts.fetch_add(1, Ordering::SeqCst);
                Ok(loaded_image())
            })
        })
    };
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![local_request(
        /*cell_id*/ 9,
        /*ordinal*/ 0,
        dir.path().join("diagram.png"),
    )]);
    let placement = update.placed[0].clone();
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator =
        MediaLoadCoordinator::with_loader(frame_requester, /*max_concurrent*/ 1, loader);
    coordinator.reconcile(&update, MediaPlacementDomain::Active);

    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("first load should request a frame")
        .expect("frame requester should remain connected");
    coordinator.poll_completed();
    assert!(matches!(
        coordinator.image_state(&placement),
        MediaImageState::Ready(_)
    ));

    assert_eq!(coordinator.clear_cache(), 1);
    assert_eq!(
        coordinator.image_state(&placement),
        MediaImageState::Pending
    );
    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("reloaded source should request a frame")
        .expect("frame requester should remain connected");
    coordinator.poll_completed();
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    assert!(matches!(
        coordinator.image_state(&placement),
        MediaImageState::Ready(_)
    ));
}

#[tokio::test]
async fn duplicate_https_sources_share_one_in_flight_load_and_become_ready() {
    let starts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let loader = {
        let starts = Arc::clone(&starts);
        let release = Arc::clone(&release);
        Arc::new(move |_url: url::Url| -> RemoteImageLoadFuture {
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
    let active_update = registry.replace_active(vec![https_request(
        /*cell_id*/ 10,
        /*ordinal*/ 0,
        "https://images.example/diagram.png",
    )]);
    let history_update = registry.append_history(vec![https_request(
        /*cell_id*/ 11,
        /*ordinal*/ 0,
        "https://images.example/diagram.png",
    )]);
    let active = active_update.placed[0].clone();
    let history = history_update.placed[0].clone();
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator = MediaLoadCoordinator::with_loaders(
        frame_requester,
        /*max_concurrent*/ 2,
        unused_local_loader(),
        loader,
    );

    coordinator.reconcile(&active_update, MediaPlacementDomain::Active);
    coordinator.reconcile(&history_update, MediaPlacementDomain::History);
    tokio::task::yield_now().await;

    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert_eq!(coordinator.image_state(&active), MediaImageState::Pending);
    assert_eq!(coordinator.image_state(&history), MediaImageState::Pending);

    release.notify_waiters();
    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("completed remote load should request a frame")
        .expect("frame requester should remain connected");
    assert_eq!(
        coordinator.poll_completed(),
        MediaLoadCompletion {
            active_ready: true,
            history_ready: true,
        }
    );
    assert!(matches!(
        coordinator.image_state(&active),
        MediaImageState::Ready(_)
    ));
    assert!(matches!(
        coordinator.image_state(&history),
        MediaImageState::Ready(_)
    ));
}

#[tokio::test]
async fn failed_https_load_becomes_unavailable() {
    let loader = Arc::new(move |_url: url::Url| -> RemoteImageLoadFuture {
        Box::pin(async move { Err(RemoteImageDownloadError::HttpStatus { status: 404 }) })
    });
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![https_request(
        /*cell_id*/ 12,
        /*ordinal*/ 0,
        "https://images.example/missing.png",
    )]);
    let placement = update.placed[0].clone();
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator = MediaLoadCoordinator::with_loaders(
        frame_requester,
        /*max_concurrent*/ 2,
        unused_local_loader(),
        loader,
    );

    coordinator.reconcile(&update, MediaPlacementDomain::Active);
    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("failed remote load should request a frame")
        .expect("frame requester should remain connected");

    assert_eq!(coordinator.poll_completed(), MediaLoadCompletion::default());
    assert_eq!(
        coordinator.image_state(&placement),
        MediaImageState::Unavailable
    );
}

#[tokio::test]
async fn local_and_https_loads_share_the_concurrency_limit() {
    let dir = tempfile::tempdir().expect("temporary image directory");
    let starts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let local_loader = {
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
    let remote_loader = {
        let starts = Arc::clone(&starts);
        let release = Arc::clone(&release);
        Arc::new(move |_url: url::Url| -> RemoteImageLoadFuture {
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
        local_request(
            /*cell_id*/ 13,
            /*ordinal*/ 0,
            dir.path().join("local.png"),
        ),
        https_request(
            /*cell_id*/ 14,
            /*ordinal*/ 0,
            "https://images.example/remote.png",
        ),
    ]);
    let (frame_requester, _frame_rx) = FrameRequester::test_channel();
    let mut coordinator = MediaLoadCoordinator::with_loaders(
        frame_requester,
        /*max_concurrent*/ 1,
        local_loader,
        remote_loader,
    );

    coordinator.reconcile(&update, MediaPlacementDomain::Active);
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(starts.load(Ordering::SeqCst), 1);

    release.notify_one();
    timeout(Duration::from_secs(1), async {
        while starts.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second source should start after the shared permit is released");
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    release.notify_waiters();
}

#[tokio::test]
async fn retired_https_placement_ignores_queued_completion() {
    let loader = Arc::new(move |_url: url::Url| -> RemoteImageLoadFuture {
        Box::pin(async move { Ok(loaded_image()) })
    });
    let mut registry = MediaPlacementRegistry::default();
    let placed = registry.replace_active(vec![https_request(
        /*cell_id*/ 15,
        /*ordinal*/ 0,
        "https://images.example/stale.png",
    )]);
    let placement = placed.placed[0].clone();
    let retired = registry.replace_active(Vec::new());
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator = MediaLoadCoordinator::with_loaders(
        frame_requester,
        /*max_concurrent*/ 2,
        unused_local_loader(),
        loader,
    );

    coordinator.reconcile(&placed, MediaPlacementDomain::Active);
    timeout(Duration::from_secs(1), frame_rx.recv())
        .await
        .expect("completed remote load should request a frame")
        .expect("frame requester should remain connected");
    coordinator.reconcile(&retired, MediaPlacementDomain::Active);

    assert_eq!(coordinator.poll_completed(), MediaLoadCompletion::default());
    assert_eq!(
        coordinator.image_state(&placement),
        MediaImageState::Unavailable
    );
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
async fn completed_latex_render_requests_frame_and_becomes_ready() {
    let mut registry = MediaPlacementRegistry::default();
    let update = registry.replace_active(vec![latex_request(
        /*cell_id*/ 31,
        /*ordinal*/ 0,
        "\\frac{1}{s+1}",
    )]);
    let placement = update.placed[0].clone();
    let (frame_requester, mut frame_rx) = FrameRequester::test_channel();
    let mut coordinator = MediaLoadCoordinator::new(frame_requester);

    coordinator.reconcile(&update, MediaPlacementDomain::Active);
    timeout(Duration::from_secs(2), frame_rx.recv())
        .await
        .expect("completed LaTeX render should request a frame")
        .expect("frame requester should remain connected");

    assert_eq!(
        coordinator.poll_completed(),
        MediaLoadCompletion {
            active_ready: true,
            history_ready: false,
        }
    );
    let image = coordinator
        .ready_image(&placement)
        .expect("rendered LaTeX should become ready");
    assert!(image.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(!image.can_use_source_file);
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
