use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::ImageSource;
use super::MediaId;
use super::MediaNode;
use super::MediaPlacementUpdate;
use super::local_loader::LoadedLocalImage;
use super::local_loader::LocalImageLimits;
use super::local_loader::LocalImageLoadError;
use super::local_loader::LocalImageLoader;
use super::local_loader::LocalImageRenderParams;
use super::placement::MediaAnchor;
use super::placement::RegisteredMediaPlacement;
use super::resolve_image_source;
use crate::tui::FrameRequester;

const DEFAULT_MAX_CONCURRENT_LOADS: usize = 2;

pub(crate) type LocalImageLoadFuture =
    Pin<Box<dyn Future<Output = Result<LoadedLocalImage, LocalImageLoadError>> + Send + 'static>>;
pub(crate) type LocalImageLoadFn = dyn Fn(PathBuf) -> LocalImageLoadFuture + Send + Sync;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediaPlacementDomain {
    Active,
    History,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MediaLoadCompletion {
    pub(crate) active_ready: bool,
    pub(crate) history_ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MediaImageState {
    Ready(LoadedLocalImage),
    Pending,
    Unavailable,
}

#[derive(Clone, Copy, Debug)]
struct MediaWaiter {
    id: MediaId,
    domain: MediaPlacementDomain,
}

#[derive(Debug)]
enum LoadOutcome {
    Ready(LoadedLocalImage),
    Failed(LocalImageLoadError),
}

struct SourceLoad {
    generation: u64,
    waiters: HashMap<MediaAnchor, MediaWaiter>,
    outcome: Option<LoadOutcome>,
    task: Option<JoinHandle<()>>,
}

struct CompletedLoad {
    path: PathBuf,
    generation: u64,
    result: Result<LoadedLocalImage, LocalImageLoadError>,
}

/// Coordinates bounded local image work outside the synchronous draw and history writer paths.
///
/// Placements for the same normalized source share one worker. Stable media anchors retain ready
/// results across redraw and reflow, while retirement detaches waiters and aborts tasks that no
/// longer have a consumer. A generation check rejects completions queued before retirement.
pub(crate) struct MediaLoadCoordinator {
    loader: Arc<LocalImageLoadFn>,
    semaphore: Arc<Semaphore>,
    frame_requester: FrameRequester,
    completion_tx: mpsc::UnboundedSender<CompletedLoad>,
    completion_rx: mpsc::UnboundedReceiver<CompletedLoad>,
    sources: HashMap<PathBuf, SourceLoad>,
    anchor_paths: HashMap<MediaAnchor, PathBuf>,
    id_anchors: HashMap<MediaId, MediaAnchor>,
    next_generation: u64,
}

impl MediaLoadCoordinator {
    pub(crate) fn new(frame_requester: FrameRequester) -> Self {
        let loader = LocalImageLoader::new(LocalImageLimits::default());
        let loader = Arc::new(move |path: PathBuf| -> LocalImageLoadFuture {
            let loader = loader.clone();
            Box::pin(async move {
                loader
                    .load_image(&path, LocalImageRenderParams::default())
                    .await
            })
        });
        Self::with_loader(frame_requester, DEFAULT_MAX_CONCURRENT_LOADS, loader)
    }

    pub(crate) fn with_loader(
        frame_requester: FrameRequester,
        max_concurrent: usize,
        loader: Arc<LocalImageLoadFn>,
    ) -> Self {
        let (completion_tx, completion_rx) = mpsc::unbounded_channel();
        Self {
            loader,
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
            frame_requester,
            completion_tx,
            completion_rx,
            sources: HashMap::new(),
            anchor_paths: HashMap::new(),
            id_anchors: HashMap::new(),
            next_generation: 0,
        }
    }

    pub(crate) fn reconcile(
        &mut self,
        update: &MediaPlacementUpdate,
        domain: MediaPlacementDomain,
    ) {
        for id in &update.retired {
            self.detach_id(*id);
        }
        for placement in &update.placed {
            self.attach(placement, domain);
        }
    }

    pub(crate) fn poll_completed(&mut self) -> MediaLoadCompletion {
        let mut completion = MediaLoadCompletion::default();
        while let Ok(loaded) = self.completion_rx.try_recv() {
            let Some(source) = self.sources.get_mut(&loaded.path) else {
                continue;
            };
            if source.generation != loaded.generation {
                continue;
            }
            source.task = None;
            match loaded.result {
                Ok(image) => {
                    for waiter in source.waiters.values() {
                        match waiter.domain {
                            MediaPlacementDomain::Active => completion.active_ready = true,
                            MediaPlacementDomain::History => completion.history_ready = true,
                        }
                    }
                    source.outcome = Some(LoadOutcome::Ready(image));
                }
                Err(error) => {
                    tracing::debug!(path = %loaded.path.display(), %error, "failed to prepare local chat image");
                    source.outcome = Some(LoadOutcome::Failed(error));
                }
            }
        }
        completion
    }

    pub(crate) fn image_state(&self, placement: &RegisteredMediaPlacement) -> MediaImageState {
        let Some(path) = self.anchor_paths.get(&placement.request.anchor) else {
            return MediaImageState::Unavailable;
        };
        let Some(source) = self.sources.get(path) else {
            return MediaImageState::Unavailable;
        };
        let Some(waiter) = source.waiters.get(&placement.request.anchor) else {
            return MediaImageState::Unavailable;
        };
        if waiter.id != placement.id {
            return MediaImageState::Unavailable;
        }
        match source.outcome.as_ref() {
            Some(LoadOutcome::Ready(image)) => MediaImageState::Ready(image.clone()),
            Some(LoadOutcome::Failed(error)) => {
                let _ = error;
                MediaImageState::Unavailable
            }
            None => MediaImageState::Pending,
        }
    }

    #[cfg(test)]
    pub(crate) fn ready_image(
        &self,
        placement: &RegisteredMediaPlacement,
    ) -> Option<LoadedLocalImage> {
        match self.image_state(placement) {
            MediaImageState::Ready(image) => Some(image),
            MediaImageState::Pending | MediaImageState::Unavailable => None,
        }
    }

    fn attach(&mut self, placement: &RegisteredMediaPlacement, domain: MediaPlacementDomain) {
        let source = match &placement.request.request.node {
            MediaNode::Image { source, .. } => source,
        };
        let Ok(ImageSource::Local(path)) = resolve_image_source(source) else {
            self.detach_id(placement.id);
            return;
        };
        let anchor = placement.request.anchor;
        if self
            .anchor_paths
            .get(&anchor)
            .is_some_and(|current| current != &path)
        {
            self.detach_anchor(anchor);
        }
        if self
            .id_anchors
            .get(&placement.id)
            .is_some_and(|current| *current != anchor)
        {
            self.detach_id(placement.id);
        }

        self.anchor_paths.insert(anchor, path.clone());
        self.id_anchors.insert(placement.id, anchor);
        if let Some(source) = self.sources.get_mut(&path) {
            source.waiters.insert(
                anchor,
                MediaWaiter {
                    id: placement.id,
                    domain,
                },
            );
            return;
        }

        self.next_generation = self.next_generation.wrapping_add(1);
        let generation = self.next_generation;
        let task = self.spawn_load(path.clone(), generation);
        self.sources.insert(
            path,
            SourceLoad {
                generation,
                waiters: HashMap::from([(
                    anchor,
                    MediaWaiter {
                        id: placement.id,
                        domain,
                    },
                )]),
                outcome: None,
                task: Some(task),
            },
        );
    }

    fn spawn_load(&self, path: PathBuf, generation: u64) -> JoinHandle<()> {
        let loader = Arc::clone(&self.loader);
        let semaphore = Arc::clone(&self.semaphore);
        let completion_tx = self.completion_tx.clone();
        let frame_requester = self.frame_requester.clone();
        tokio::spawn(async move {
            let Ok(_permit) = semaphore.acquire_owned().await else {
                return;
            };
            let result = loader(path.clone()).await;
            if completion_tx
                .send(CompletedLoad {
                    path,
                    generation,
                    result,
                })
                .is_ok()
            {
                frame_requester.schedule_frame();
            }
        })
    }

    fn detach_id(&mut self, id: MediaId) {
        let Some(anchor) = self.id_anchors.remove(&id) else {
            return;
        };
        self.detach_anchor(anchor);
    }

    fn detach_anchor(&mut self, anchor: MediaAnchor) {
        self.id_anchors.retain(|_, current| *current != anchor);
        let Some(path) = self.anchor_paths.remove(&anchor) else {
            return;
        };
        let remove_source = self.sources.get_mut(&path).is_some_and(|source| {
            source.waiters.remove(&anchor);
            source.waiters.is_empty()
        });
        if remove_source
            && let Some(mut source) = self.sources.remove(&path)
            && let Some(task) = source.task.take()
        {
            task.abort();
        }
    }
}

impl Drop for MediaLoadCoordinator {
    fn drop(&mut self) {
        for source in self.sources.values_mut() {
            if let Some(task) = source.task.take() {
                task.abort();
            }
        }
    }
}
