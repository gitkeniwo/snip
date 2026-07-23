use std::path::PathBuf;
use std::time::Instant;

use ratatui::widgets::ListState;
use uuid::Uuid;

use crate::config::TuiThemeSetting;
use crate::domain::CatalogSnapshot;
use crate::filesystem::Library;
use crate::search::MemoryIndex;

use super::super::editor::EditRequest;
use super::super::highlight::Highlighter;
use super::super::icons::IconMode;
use super::super::layout::LayoutRects;
use super::super::modal::Modal;
use super::super::preview::PreviewCache;
use super::super::selection::PreviewSelection;
use super::super::state::{
    Filter, Pane, SearchState, SidebarState, SortMode, StatusMessage, VisibleRow,
};
use super::super::theme::TuiTheme;
use super::super::trash::TrashState;

#[derive(Clone, Debug)]
pub enum Effect {
    SpawnEditor(EditRequest),
    ForceSave(EditRequest),
    CopyToClipboard { text: String, label: String },
    OpenInVsCode { path: PathBuf },
}

pub struct App {
    pub library: Library,
    pub catalog: CatalogSnapshot,
    pub index: MemoryIndex,
    pub focus: Pane,
    pub sidebar: SidebarState,
    pub filter: Filter,
    pub search: SearchState,
    pub visible: Vec<VisibleRow>,
    pub list_state: ListState,
    pub selected_id: Option<Uuid>,
    pub fragment_index: usize,
    pub preview_scroll: u16,
    pub show_line_numbers: bool,
    pub sort: SortMode,
    pub layout: LayoutRects,
    pub preview: PreviewCache,
    pub preview_selection: PreviewSelection,
    pub highlighter: Highlighter,
    pub theme: TuiTheme,
    pub theme_setting: TuiThemeSetting,
    pub theme_overrides: toml::Table,
    pub icon_mode: IconMode,
    pub theme_checked_at: Instant,
    pub status: Option<StatusMessage>,
    pub modal: Option<Modal>,
    pub trash: TrashState,
    pub should_quit: bool,
    pub editor_cmd: Option<String>,
    pub vscode_cmd: Option<String>,
    pub show_help: bool,
    pub default_language: String,
    pub default_folder: Option<String>,
    pub default_tags: Vec<String>,
    pub(super) last_click: Option<(usize, Instant)>,
}
