use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use ratatui::widgets::ListState;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pane {
    Sidebar,
    List,
    Preview,
}

/// Shared with `snip list --sort` and the `[tui] sort` config key so both surfaces
/// name and apply the same orders.
pub use crate::sort::SortMode;

/// Top-bar badge for the active sort. `Manual` is the default, so it stays unlabelled.
pub fn sort_indicator(sort: SortMode) -> Option<&'static str> {
    match sort {
        SortMode::Manual => None,
        SortMode::Title => Some("↑ title"),
        SortMode::Modified => Some("↓ modified"),
        SortMode::Created => Some("↓ created"),
    }
}

impl Pane {
    pub fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::List,
            Self::List => Self::Preview,
            Self::Preview => Self::Sidebar,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Sidebar => Self::Preview,
            Self::List => Self::Sidebar,
            Self::Preview => Self::List,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidebarItem {
    All,
    Uncategorized,
    Folder(String),
    Trash,
    Tag(String),
    Header,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarRow {
    pub item: SidebarItem,
    pub label: String,
    pub depth: usize,
    pub count: usize,
    pub has_children: bool,
    pub expanded: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SidebarState {
    pub rows: Vec<SidebarRow>,
    pub list_state: ListState,
    pub expanded: BTreeSet<String>,
}

impl SidebarState {
    pub fn selected(&self) -> Option<&SidebarRow> {
        self.list_state
            .selected()
            .and_then(|index| self.rows.get(index))
    }

    pub fn select_first_actionable(&mut self) {
        let index = self
            .rows
            .iter()
            .position(|row| row.item != SidebarItem::Header);
        self.list_state.select(index);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Filter {
    pub uncategorized: bool,
    pub folder: Option<String>,
    pub tag: Option<String>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        !self.uncategorized && self.folder.is_none() && self.tag.is_none()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleRow {
    pub snippet_id: Uuid,
    pub excerpt: Option<String>,
    pub score: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusLevel {
    Info,
    Error,
}

#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub text: String,
    pub level: StatusLevel,
    expires_at: Instant,
}

impl StatusMessage {
    pub fn new(text: impl Into<String>, level: StatusLevel) -> Self {
        Self {
            text: text.into(),
            level,
            expires_at: Instant::now() + Duration::from_secs(5),
        }
    }

    pub fn expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}
