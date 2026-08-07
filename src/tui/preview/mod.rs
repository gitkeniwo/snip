mod cache;
mod layout;
mod render;

pub use cache::{PreviewCache, PreviewDocument};
pub use layout::{WrappedPreview, jump_paragraph};
pub(crate) use render::fragment_label;
pub use render::{draw_preview, draw_preview_of};

/// What the preview pane is currently showing.
///
/// The README is a selectable target but never a fragment: it has no manifest
/// entry, no id, and no place in the fragment count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreviewTarget {
    Fragment(usize),
    Readme,
}

impl PreviewTarget {
    /// The fragment index, or `None` when the README is selected.
    pub fn fragment_index(self) -> Option<usize> {
        match self {
            Self::Fragment(index) => Some(index),
            Self::Readme => None,
        }
    }
}

impl Default for PreviewTarget {
    fn default() -> Self {
        Self::Fragment(0)
    }
}

/// A README the tree can show: present and non-empty.
pub fn has_readme(snippet: &crate::domain::Snippet) -> bool {
    snippet
        .readme
        .as_deref()
        .is_some_and(|readme| !readme.is_empty())
}
