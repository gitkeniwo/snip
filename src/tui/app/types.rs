use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use ratatui::widgets::ListState;
use uuid::Uuid;

use crate::config::{EditorCwdSetting, GitConfig, TuiConfig, TuiDensitySetting, TuiThemeSetting};
use crate::domain::CatalogSnapshot;
use crate::filesystem::Library;
use crate::git;
use crate::keys::Keymap;
use crate::search::MemoryIndex;
#[cfg(test)]
use crate::tui::command::CommandId;

use super::super::editor::EditRequest;
use super::super::event::AppEvent;
use super::super::gist_panel::GistBadge;
use super::super::help::HelpState;
use super::super::highlight::Highlighter;
use super::super::icons::IconMode;
use super::super::layout::LayoutRects;
use super::super::modal::Modal;
use super::super::palette::PaletteState;
use super::super::preview::{PreviewCache, PreviewTarget};
use super::super::selection::PreviewSelection;
use super::super::state::{
    Filter, Pane, SearchState, SidebarState, SortMode, StatusMessage, VisibleRow,
};
use super::super::theme::{Appearance, TuiTheme};
use super::super::trash::TrashState;

#[derive(Clone, Debug)]
pub struct ThemePreviewState {
    pub original_name: String,
    pub original_source: crate::theme::Theme,
    pub original_tui: TuiTheme,
}

#[derive(Clone, Debug)]
pub enum Effect {
    SpawnEditor {
        request: EditRequest,
        cwd: Option<PathBuf>,
    },
    ForceSave(EditRequest),
    CopyToClipboard {
        text: String,
        label: String,
    },
    OpenInVsCode {
        path: PathBuf,
    },
    RunGit(git::GitAction),
}

pub struct GitState {
    pub repo: Option<git::Repo>,
    pub unavailable: Option<git::Unavailable>,
    pub status: Option<git::Status>,
    pub error: Option<String>,
    pub open: bool,
    pub auto_commit_interval: u32,
    pub auto_push: bool,
    pub auto_pull: bool,
    pub backup_on_quit: bool,
    pub auto_backup_paused: bool,
    pub last_commit_error: Option<String>,
    pub last_push_error: Option<String>,
    pub last_fetch_error: Option<String>,
    pub operation_queued: bool,
    pub auto_attempted_at: Option<Instant>,
    pub push_attempted_at: Option<Instant>,
    pub push_in_flight: bool,
    pub pull_in_flight: bool,
    pub fetch_in_flight: bool,
    pub fetched_at: Option<Instant>,
    pub sender: Option<Sender<AppEvent>>,
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
            auto_push: config.auto_push,
            auto_pull: config.auto_pull,
            backup_on_quit: config.backup_on_quit,
            auto_backup_paused: false,
            last_commit_error: None,
            last_push_error: None,
            last_fetch_error: None,
            operation_queued: false,
            auto_attempted_at: None,
            push_attempted_at: None,
            push_in_flight: false,
            pull_in_flight: false,
            fetch_in_flight: false,
            fetched_at: None,
            sender: None,
            checked_at: Instant::now(),
            interval: Duration::from_secs(5),
        }
    }

    pub(super) fn reprobe(&mut self, library: &Library) {
        let open = self.open;
        let auto_backup_paused = self.auto_backup_paused;
        let last_commit_error = self.last_commit_error.take();
        let last_push_error = self.last_push_error.take();
        let last_fetch_error = self.last_fetch_error.take();
        let sender = self.sender.clone();
        let push_attempted_at = self.push_attempted_at;
        let push_in_flight = self.push_in_flight;
        let pull_in_flight = self.pull_in_flight;
        let fetch_in_flight = self.fetch_in_flight;
        let fetched_at = self.fetched_at;
        let config = GitConfig {
            auto_commit_interval: self.auto_commit_interval,
            auto_push: self.auto_push,
            auto_pull: self.auto_pull,
            backup_on_quit: self.backup_on_quit,
            ..GitConfig::default()
        };
        *self = Self::probe(library, &config);
        self.open = open;
        self.auto_backup_paused = auto_backup_paused;
        self.last_commit_error = last_commit_error;
        self.last_push_error = last_push_error;
        self.last_fetch_error = last_fetch_error;
        self.sender = sender;
        self.push_attempted_at = push_attempted_at;
        self.push_in_flight = push_in_flight;
        self.pull_in_flight = pull_in_flight;
        self.fetch_in_flight = fetch_in_flight;
        self.fetched_at = fetched_at;
    }
}

#[derive(Clone, Debug, Default)]
pub struct GistState {
    pub open: bool,
    pub in_flight: bool,
    pub unavailable: Option<crate::gist::gh::Unavailable>,
    pub last_error: Option<String>,
    pub sender: Option<Sender<AppEvent>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FragmentGrab {
    /// Index the fragment occupied when it was grabbed.
    pub origin: usize,
    /// Index it is currently hovering over; equals `origin` until the user moves it.
    pub current: usize,
}

/// Display order while a fragment is grabbed: `origin` lifted out and
/// re-inserted at `current`. Returns original indices in display order.
pub fn grab_order(total: usize, origin: usize, current: usize) -> Vec<usize> {
    let mut order = (0..total).collect::<Vec<_>>();
    if origin < total && current < total && origin != current {
        let grabbed = order.remove(origin);
        order.insert(current, grabbed);
    }
    order
}

pub struct App {
    pub config_path: PathBuf,
    pub library: Library,
    pub catalog: CatalogSnapshot,
    pub index: MemoryIndex,
    pub gist_badges: HashMap<Uuid, GistBadge>,
    pub focus: Pane,
    pub sidebar: SidebarState,
    pub filter: Filter,
    pub search: SearchState,
    pub visible: Vec<VisibleRow>,
    pub list_state: ListState,
    pub selected_id: Option<Uuid>,
    pub preview_target: PreviewTarget,
    pub fragment_grab: Option<FragmentGrab>,
    pub preview_scroll: u16,
    pub show_line_numbers: bool,
    pub simplified_ui: bool,
    /// Session-only, unlike `density` and `line_numbers`: a narrow terminal is
    /// a fact about this connection, not a preference to carry to the next machine.
    pub show_sidebar: bool,
    pub fragments_expanded: bool,
    pub sort: SortMode,
    pub density: TuiDensitySetting,
    pub layout: LayoutRects,
    pub preview: PreviewCache,
    pub preview_selection: PreviewSelection,
    pub highlighter: Highlighter,
    pub theme: TuiTheme,
    pub theme_source: crate::theme::Theme,
    pub theme_name: String,
    pub theme_preview: Option<ThemePreviewState>,
    pub theme_setting: TuiThemeSetting,
    pub theme_config: TuiConfig,
    pub theme_overrides: toml::Table,
    /// Set by `view.cycle-appearance`. Session-only: deliberately never written
    /// to the config, unlike `density` and `line_numbers`. It corrects a wrong
    /// reading of the host's appearance for one terminal, which is not a
    /// preference worth carrying to the next machine.
    pub appearance_override: Option<Appearance>,
    /// `SNIP_TUI_THEME` as read once at startup, so an explicit override can
    /// take its place rather than fight it.
    pub theme_env: Option<String>,
    pub icon_mode: IconMode,
    pub git: GitState,
    pub gist: GistState,
    pub theme_checked_at: Instant,
    pub status: Option<StatusMessage>,
    pub modal: Option<Modal>,
    pub palette: PaletteState,
    pub keymap: Keymap,
    pub(crate) session_state_extra: toml::Table,
    pub trash: TrashState,
    pub should_quit: bool,
    pub pending_quit: bool,
    pub editor_cmd: Option<String>,
    pub editor_cwd: EditorCwdSetting,
    pub vscode_cmd: Option<String>,
    pub show_help: bool,
    pub help: HelpState,
    pub default_language: String,
    pub default_folder: Option<String>,
    pub default_tags: Vec<String>,
    pub(super) last_click: Option<(usize, Instant)>,
    #[cfg(test)]
    pub(crate) last_command: Option<CommandId>,
}

#[cfg(test)]
mod tests {
    use super::grab_order;

    #[test]
    fn grab_order_moves_down_up_and_preserves_identity() {
        assert_eq!(grab_order(5, 1, 3), vec![0, 2, 3, 1, 4]);
        assert_eq!(grab_order(5, 3, 1), vec![0, 3, 1, 2, 4]);
        assert_eq!(grab_order(5, 2, 2), vec![0, 1, 2, 3, 4]);
        assert_eq!(grab_order(1, 0, 0), vec![0]);
    }
}
