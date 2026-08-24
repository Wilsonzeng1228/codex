use std::num::NonZeroU64;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use super::MediaId;
use super::MediaNode;
use super::MediaPlacementRequest;

static NEXT_MEDIA_CELL_ID: AtomicU64 = AtomicU64::new(1);

/// Identifies one source-backed transcript cell for the lifetime of this process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MediaCellId(NonZeroU64);

impl MediaCellId {
    pub(crate) const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub(crate) fn allocate() -> Self {
        loop {
            let value = NEXT_MEDIA_CELL_ID.fetch_add(1, Ordering::Relaxed);
            if let Some(id) = Self::new(value) {
                return id;
            }
        }
    }
}

/// Stable identity of one image inside one transcript cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MediaAnchor {
    pub(crate) cell_id: MediaCellId,
    pub(crate) ordinal: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnchoredMediaPlacementRequest {
    pub(crate) anchor: MediaAnchor,
    pub(crate) request: MediaPlacementRequest,
}

impl AnchoredMediaPlacementRequest {
    pub(crate) fn new(cell_id: MediaCellId, request: MediaPlacementRequest) -> Self {
        let ordinal = match &request.node {
            MediaNode::Image { ordinal, .. } | MediaNode::Latex { ordinal, .. } => *ordinal,
        };
        Self {
            anchor: MediaAnchor { cell_id, ordinal },
            request,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegisteredMediaPlacement {
    pub(crate) id: MediaId,
    pub(crate) request: AnchoredMediaPlacementRequest,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MediaPlacementUpdate {
    pub(crate) retired: Vec<MediaId>,
    /// Placements that must be emitted again. Reusing an ID lets Kitty replace it atomically.
    pub(crate) placed: Vec<RegisteredMediaPlacement>,
}

/// Owns terminal-media identities independently from copyable transcript lines.
///
/// Stable anchors let active placements move into history without changing image IDs. Scoped
/// history replacement rebuilds only the transcript suffix affected by resize/reflow, preserving
/// placements that already live in older terminal scrollback.
#[derive(Debug, Default)]
pub(crate) struct MediaPlacementRegistry {
    next_id: u32,
    active: Vec<RegisteredMediaPlacement>,
    history: Vec<RegisteredMediaPlacement>,
}

impl MediaPlacementRegistry {
    pub(crate) fn replace_active(
        &mut self,
        requests: Vec<AnchoredMediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let mut previous = std::mem::take(&mut self.active);
        let placed = self.register_reusing(requests, &mut previous);
        let retired = previous.into_iter().map(|placement| placement.id).collect();
        self.active.clone_from(&placed);
        MediaPlacementUpdate { retired, placed }
    }

    pub(crate) fn append_history(
        &mut self,
        requests: Vec<AnchoredMediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let mut placed = Vec::with_capacity(requests.len());
        for request in requests {
            let existing = take_by_anchor(&mut self.active, request.anchor)
                .or_else(|| take_by_anchor(&mut self.history, request.anchor));
            let id = existing
                .map(|placement| placement.id)
                .unwrap_or_else(|| self.allocate_id());
            placed.push(RegisteredMediaPlacement { id, request });
        }
        self.history.extend(placed.iter().cloned());
        MediaPlacementUpdate {
            retired: Vec::new(),
            placed,
        }
    }

    pub(crate) fn replace_history(
        &mut self,
        requests: Vec<AnchoredMediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let mut scope = self
            .history
            .iter()
            .map(|placement| placement.request.anchor.cell_id)
            .collect::<Vec<_>>();
        scope.extend(requests.iter().map(|request| request.anchor.cell_id));
        self.replace_history_scope(&scope, requests)
    }

    pub(crate) fn replace_history_scope(
        &mut self,
        scope: &[MediaCellId],
        requests: Vec<AnchoredMediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let mut previous = Vec::new();
        self.history.retain(|placement| {
            if scope.contains(&placement.request.anchor.cell_id) {
                previous.push(placement.clone());
                false
            } else {
                true
            }
        });

        let placed = self.register_reusing(requests, &mut previous);
        let retired = previous.into_iter().map(|placement| placement.id).collect();
        self.history.extend(placed.iter().cloned());
        MediaPlacementUpdate { retired, placed }
    }

    #[cfg(test)]
    pub(crate) fn active(&self) -> &[RegisteredMediaPlacement] {
        &self.active
    }

    pub(crate) fn history(&self) -> &[RegisteredMediaPlacement] {
        &self.history
    }

    fn register_reusing(
        &mut self,
        requests: Vec<AnchoredMediaPlacementRequest>,
        previous: &mut Vec<RegisteredMediaPlacement>,
    ) -> Vec<RegisteredMediaPlacement> {
        requests
            .into_iter()
            .map(|request| {
                let id = take_by_anchor(previous, request.anchor)
                    .map(|placement| placement.id)
                    .unwrap_or_else(|| self.allocate_id());
                RegisteredMediaPlacement { id, request }
            })
            .collect()
    }

    fn allocate_id(&mut self) -> MediaId {
        loop {
            self.next_id = self.next_id.wrapping_add(1);
            if let Some(id) = MediaId::new(self.next_id)
                && self
                    .active
                    .iter()
                    .chain(&self.history)
                    .all(|placement| placement.id != id)
            {
                return id;
            }
        }
    }
}

fn take_by_anchor(
    placements: &mut Vec<RegisteredMediaPlacement>,
    anchor: MediaAnchor,
) -> Option<RegisteredMediaPlacement> {
    let index = placements
        .iter()
        .position(|placement| placement.request.anchor == anchor)?;
    Some(placements.remove(index))
}
