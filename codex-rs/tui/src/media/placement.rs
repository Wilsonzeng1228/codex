use super::MediaId;
use super::MediaPlacementRequest;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegisteredMediaPlacement {
    pub(crate) id: MediaId,
    pub(crate) request: MediaPlacementRequest,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MediaPlacementUpdate {
    pub(crate) retired: Vec<MediaId>,
    pub(crate) added: Vec<RegisteredMediaPlacement>,
}

/// Owns terminal-media identities independently from copyable transcript lines.
///
/// Active placements are replaced on every completed frame. History placements survive active
/// redraws and are replaced only when scrollback is cleared or rebuilt from transcript source.
#[derive(Debug, Default)]
pub(crate) struct MediaPlacementRegistry {
    next_id: u32,
    active: Vec<RegisteredMediaPlacement>,
    history: Vec<RegisteredMediaPlacement>,
}

impl MediaPlacementRegistry {
    pub(crate) fn replace_active(
        &mut self,
        requests: Vec<MediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let retired = self
            .active
            .drain(..)
            .map(|placement| placement.id)
            .collect();
        let added = self.register(requests);
        self.active.clone_from(&added);
        MediaPlacementUpdate { retired, added }
    }

    pub(crate) fn append_history(
        &mut self,
        requests: Vec<MediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let added = self.register(requests);
        self.history.extend(added.iter().cloned());
        MediaPlacementUpdate {
            retired: Vec::new(),
            added,
        }
    }

    pub(crate) fn replace_history(
        &mut self,
        requests: Vec<MediaPlacementRequest>,
    ) -> MediaPlacementUpdate {
        let retired = self
            .history
            .drain(..)
            .map(|placement| placement.id)
            .collect();
        let added = self.register(requests);
        self.history.clone_from(&added);
        MediaPlacementUpdate { retired, added }
    }

    #[cfg(test)]
    pub(crate) fn active(&self) -> &[RegisteredMediaPlacement] {
        &self.active
    }

    #[cfg(test)]
    pub(crate) fn history(&self) -> &[RegisteredMediaPlacement] {
        &self.history
    }

    fn register(&mut self, requests: Vec<MediaPlacementRequest>) -> Vec<RegisteredMediaPlacement> {
        requests
            .into_iter()
            .map(|request| RegisteredMediaPlacement {
                id: self.allocate_id(),
                request,
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
