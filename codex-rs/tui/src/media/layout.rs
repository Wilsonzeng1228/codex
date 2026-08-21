use std::num::NonZeroU16;

use ratatui::layout::Rect;

use super::node::MediaNode;
use crate::terminal_hyperlinks::HyperlinkLine;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MediaPlaceholderRows(NonZeroU16);

impl MediaPlaceholderRows {
    pub(crate) const fn get(self) -> u16 {
        self.0.get()
    }
}

impl TryFrom<u16> for MediaPlaceholderRows {
    type Error = &'static str;

    fn try_from(rows: u16) -> Result<Self, Self::Error> {
        NonZeroU16::new(rows)
            .map(Self)
            .ok_or("media placeholder height must be non-zero")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaPlacementRequest {
    pub(crate) node: MediaNode,
    /// Cell-relative rectangle reserved for the terminal image.
    pub(crate) rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaLayout {
    pub(crate) lines: Vec<HyperlinkLine>,
    pub(crate) placements: Vec<MediaPlacementRequest>,
}

impl MediaLayout {
    pub(crate) fn text_only(lines: Vec<HyperlinkLine>) -> Self {
        Self {
            lines,
            placements: Vec::new(),
        }
    }
}
