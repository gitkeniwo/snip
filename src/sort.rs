use std::cmp::Ordering;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::domain::Snippet;

/// Snippet ordering shared by `snip list --sort`, the `[tui] sort` config key, and
/// the TUI sort cycle, so every surface names and applies the same orders.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
pub enum SortMode {
    /// Catalog order: folder, then title, as recorded on disk.
    #[default]
    Manual,
    /// Case-insensitive title, ascending.
    Title,
    /// Most recently modified first.
    Modified,
    /// Most recently created first.
    Created,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            Self::Manual => Self::Title,
            Self::Title => Self::Modified,
            Self::Modified => Self::Created,
            Self::Created => Self::Manual,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Title => "title",
            Self::Modified => "modified",
            Self::Created => "created",
        }
    }

    /// Pinned snippets always sort first; the mode only breaks ties after that.
    pub fn compare(self, left: &Snippet, right: &Snippet) -> Ordering {
        (!left.pinned)
            .cmp(&(!right.pinned))
            .then_with(|| match self {
                Self::Manual => Ordering::Equal,
                Self::Title => left.title.to_lowercase().cmp(&right.title.to_lowercase()),
                Self::Modified => compare_optional_desc(&left.modified_at, &right.modified_at),
                // `created_at` is emitted in RFC3339 UTC form by snip, so lexical ordering is
                // chronological. Imported snippets with mixed offsets may differ slightly.
                Self::Created => right.created_at.cmp(&left.created_at),
            })
    }
}

/// Newest first, with missing timestamps last.
fn compare_optional_desc(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
