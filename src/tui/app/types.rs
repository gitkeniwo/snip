use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::widgets::ListState;
use uuid::Uuid;

use crate::config::{GitConfig, TuiThemeSetting};
use crate::domain::CatalogSnapshot;
use crate::filesystem::Library;
use crate::git;
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
    RunGit(git::GitAction),
}

pub struct GitState {
    pub repo: Option<git::Repo>,
    pub unavailable: Option<git::Unavailable>,
    pub status: Option<git::Status>,
    pub error: Option<String>,
    pub open: bool,
    pub auto_commit_interval: u32,
    pub backup_on_quit: bool,
    pub auto_backup_paused: bool,
    pub last_auto_error: Option<String>,
    pub operation_queued: bool,
    pub auto_attempted_at: Option<Instant>,
    pub(crate) checked_at: Instant,
    pub(crate) interval: Duration,
}

impl GitState {
    pub(super) fn probe(library: &Library, config: &GitConfig) -> Self {
        let (repo, unavailable) = match git::probe(library.root()) {
            Ok(repo) => (Some(repo), None),
            Err(unavailable) => (None, Some(unavailable)),
        };
        Self {
            repo,
            unavailable,
            status: None,
            error: None,
            open: false,
            auto_commit_interval: config.auto_commit_interval,
            backup_on_quit: config.backup_on_quit,
            auto_backup_paused: false,
            last_auto_error: None,
            operation_queued: false,
            auto_attempted_at: None,
            checked_at: Instant::now(),
            interval: Duration::from_secs(5),
        }
    }

    pub(super) fn reprobe(&mut self, library: &Library) {
        let open = self.open;
        let auto_backup_paused = self.auto_backup_paused;
        let last_auto_error = self.last_auto_error.take();
        let config = GitConfig {
            auto_commit_interval: self.auto_commit_interval,
            backup_on_quit: self.backup_on_quit,
            ..GitConfig::default()
        };
        *self = Self::probe(library, &config);
        self.open = open;
        self.auto_backup_paused = auto_backup_paused;
        self.last_auto_error = last_auto_error;
    }
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
    pub git: GitState,
    pub theme_checked_at: Instant,
    pub status: Option<StatusMessage>,
    pub modal: Option<Modal>,
    pub trash: TrashState,
    pub should_quit: bool,
    pub pending_quit: bool,
    pub editor_cmd: Option<String>,
    pub vscode_cmd: Option<String>,
    pub show_help: bool,
    pub default_language: String,
    pub default_folder: Option<String>,
    pub default_tags: Vec<String>,
    pub(super) last_click: Option<(usize, Instant)>,
}
