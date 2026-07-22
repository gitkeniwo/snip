use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::domain::{CatalogSnapshot, Snippet};
use crate::error::Result;
use crate::filesystem::Library;
use crate::search::{MemoryIndex, SearchIndex};

use super::editor::{EditOutcome, EditRequest};
use super::highlight::Highlighter;
use super::preview::PreviewCache;
use super::sidebar;
use super::state::{
    Filter, Pane, PendingPrompt, SearchState, SidebarItem, SidebarState, StatusLevel,
    StatusMessage, VisibleRow,
};
use super::theme::TuiTheme;

#[derive(Clone, Debug)]
pub enum Effect {
    SpawnEditor(EditRequest),
    ForceSave(EditRequest),
    CopyToClipboard { text: String, label: String },
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
    pub preview: PreviewCache,
    pub highlighter: Highlighter,
    pub theme: TuiTheme,
    pub theme_checked_at: Instant,
    pub status: Option<StatusMessage>,
    pub pending: Option<PendingPrompt>,
    pub should_quit: bool,
    pub editor_cmd: Option<String>,
    pub show_help: bool,
}

impl App {
    pub fn new(library: Library, config: &AppConfig) -> Result<Self> {
        let catalog = library.scan()?;
        let index = MemoryIndex::new(catalog.clone());
        let theme = TuiTheme::detect();
        let mut app = Self {
            library,
            catalog,
            index,
            focus: Pane::Sidebar,
            sidebar: SidebarState::default(),
            filter: Filter::default(),
            search: SearchState::default(),
            visible: Vec::new(),
            list_state: ListState::default(),
            selected_id: None,
            fragment_index: 0,
            preview_scroll: 0,
            preview: PreviewCache::default(),
            highlighter: Highlighter::new(theme)?,
            theme,
            theme_checked_at: Instant::now(),
            status: None,
            pending: None,
            should_quit: false,
            editor_cmd: config.editor.clone(),
            show_help: false,
        };
        sidebar::rebuild(&mut app.sidebar, &app.catalog);
        app.refresh_visible();
        Ok(app)
    }

    pub fn selected_snippet(&self) -> Option<&Snippet> {
        let id = self.selected_id?;
        self.catalog
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
    }

    pub fn set_status(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.status = Some(StatusMessage::new(text, level));
    }

    pub fn tick_status(&mut self) {
        if self.pending.is_none() && self.status.as_ref().is_some_and(StatusMessage::expired) {
            self.status = None;
        }
    }

    pub fn tick_theme(&mut self) -> Result<()> {
        if self.theme_checked_at.elapsed() < Duration::from_secs(5) {
            return Ok(());
        }
        self.theme_checked_at = Instant::now();
        let theme = TuiTheme::detect();
        if theme.appearance != self.theme.appearance {
            self.highlighter = Highlighter::new(theme)?;
            self.theme = theme;
            self.preview.invalidate();
        }
        Ok(())
    }

    pub fn rescan(&mut self) -> Result<()> {
        let catalog = self.library.scan()?;
        self.catalog = catalog.clone();
        self.index = MemoryIndex::new(catalog);
        sidebar::rebuild(&mut self.sidebar, &self.catalog);
        self.refresh_visible();
        Ok(())
    }

    pub fn refresh_visible(&mut self) {
        let old_index = self.list_state.selected().unwrap_or(0);
        let allowed = self
            .catalog
            .snippets
            .iter()
            .filter(|snippet| self.matches_filter(snippet))
            .map(|snippet| snippet.id)
            .collect::<HashSet<_>>();

        let mut visible = if self.search.query.is_empty() {
            let mut snippets = self
                .catalog
                .snippets
                .iter()
                .filter(|snippet| allowed.contains(&snippet.id))
                .collect::<Vec<_>>();
            snippets.sort_by_key(|snippet| !snippet.pinned);
            snippets
                .into_iter()
                .map(|snippet| VisibleRow {
                    snippet_id: snippet.id,
                    excerpt: None,
                    score: 0,
                })
                .collect()
        } else {
            let mut best = HashMap::<Uuid, VisibleRow>::new();
            for result in self
                .index
                .search(&self.search.query, None, self.filter.tag.as_deref())
            {
                if !allowed.contains(&result.snippet_id) {
                    continue;
                }
                best.entry(result.snippet_id).or_insert(VisibleRow {
                    snippet_id: result.snippet_id,
                    excerpt: Some(result.excerpt),
                    score: result.score,
                });
            }
            let mut rows = best.into_values().collect::<Vec<_>>();
            rows.sort_by(|left, right| {
                right.score.cmp(&left.score).then_with(|| {
                    self.title_for(left.snippet_id)
                        .to_lowercase()
                        .cmp(&self.title_for(right.snippet_id).to_lowercase())
                })
            });
            rows
        };

        let selection = self
            .selected_id
            .and_then(|id| visible.iter().position(|row| row.snippet_id == id))
            .or_else(|| (!visible.is_empty()).then(|| old_index.min(visible.len() - 1)));
        self.list_state.select(selection);
        self.selected_id = selection.map(|index| visible[index].snippet_id);
        self.visible.clear();
        self.visible.append(&mut visible);
        self.clamp_fragment();
        self.preview.invalidate();
        self.preview_scroll = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Vec::new();
        }
        if self.pending.is_some() {
            return self.handle_prompt(key);
        }
        if self.search.active {
            return self.handle_search(key);
        }
        if self.show_help {
            match key.code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Char('h') => self.focus = self.focus.previous(),
            KeyCode::Char('l') => self.focus = self.focus.next(),
            KeyCode::Char('/') => self.search.active = true,
            KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                } else if !self.search.query.is_empty() {
                    self.search.query.clear();
                    self.refresh_visible();
                } else if !self.filter.is_empty() {
                    self.filter = Filter::default();
                    self.refresh_visible();
                }
            }
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Char('r') => match self.rescan() {
                Ok(()) => self.set_status("library refreshed", StatusLevel::Info),
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            KeyCode::Char('e') => return self.edit_effect(),
            KeyCode::Char('y') => return self.copy_content_effect(),
            KeyCode::Char('Y') => return self.copy_id_effect(),
            KeyCode::Char('[') => self.previous_fragment(),
            KeyCode::Char(']') => self.next_fragment(),
            _ => self.handle_pane_key(key),
        }
        Vec::new()
    }

    pub fn handle_editor_outcome(&mut self, outcome: EditOutcome) {
        match outcome {
            EditOutcome::Unchanged => {
                self.set_status("editor closed without changes", StatusLevel::Info)
            }
            EditOutcome::Saved => {
                if let Err(error) = self.rescan() {
                    self.set_status(error.to_string(), StatusLevel::Error);
                } else {
                    self.set_status("fragment saved", StatusLevel::Info);
                }
            }
            EditOutcome::Conflict(request) => {
                self.pending = Some(PendingPrompt::ForceEdit(request));
                self.set_status(
                    "changed on disk while editing — y: overwrite, n: discard",
                    StatusLevel::Error,
                );
            }
        }
    }

    fn matches_filter(&self, snippet: &Snippet) -> bool {
        let folder_matches = self.filter.folder.as_ref().is_none_or(|folder| {
            snippet.folder == *folder || snippet.folder.starts_with(&format!("{folder}/"))
        });
        let tag_matches = self.filter.tag.as_ref().is_none_or(|tag| {
            snippet
                .tags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(tag))
        });
        folder_matches && tag_matches
    }

    fn title_for(&self, id: Uuid) -> &str {
        self.catalog
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
            .map_or("", |snippet| snippet.title.as_str())
    }

    fn handle_prompt(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let Some(PendingPrompt::ForceEdit(request)) = self.pending.take() else {
                    return Vec::new();
                };
                vec![Effect::ForceSave(request)]
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending = None;
                self.set_status("edited content discarded", StatusLevel::Info);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_search(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Enter => {
                self.search.active = false;
                self.focus = Pane::List;
            }
            KeyCode::Esc => {
                self.search.active = false;
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                self.refresh_visible();
            }
            KeyCode::Char(value)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.search.query.push(value);
                self.refresh_visible();
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_pane_key(&mut self, key: KeyEvent) {
        match self.focus {
            Pane::Sidebar => self.handle_sidebar_key(key),
            Pane::List => self.handle_list_key(key),
            Pane::Preview => self.handle_preview_key(key),
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_sidebar(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_sidebar(-1),
            KeyCode::Char('g') => self.select_sidebar(0),
            KeyCode::Char('G') => self.select_sidebar(self.sidebar.rows.len().saturating_sub(1)),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_sidebar(10)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_sidebar(-10)
            }
            KeyCode::Enter => self.apply_sidebar_filter(),
            KeyCode::Char(' ') => self.toggle_sidebar_folder(),
            _ => {}
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_list(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_list(-1),
            KeyCode::Char('g') => self.select_list(0),
            KeyCode::Char('G') => self.select_list(self.visible.len().saturating_sub(1)),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_list(10)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_list(-10)
            }
            KeyCode::Enter => self.focus = Pane::Preview,
            _ => {}
        }
    }

    fn handle_preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.preview_scroll = self.preview_scroll.saturating_add(1)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1)
            }
            KeyCode::Char('g') => self.preview_scroll = 0,
            KeyCode::Char('G') => self.preview_scroll = u16::MAX,
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.preview_scroll = self.preview_scroll.saturating_add(10)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.preview_scroll = self.preview_scroll.saturating_sub(10)
            }
            _ => {}
        }
    }

    fn move_sidebar(&mut self, delta: isize) {
        let len = self.sidebar.rows.len();
        if len == 0 {
            return;
        }
        let mut index = self.sidebar.list_state.selected().unwrap_or(0);
        loop {
            index = (index as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
            if self.sidebar.rows[index].item != SidebarItem::Header
                || index == 0
                || index + 1 == len
            {
                break;
            }
        }
        self.sidebar.list_state.select(Some(index));
        self.sync_sidebar_filter();
    }

    fn select_sidebar(&mut self, mut index: usize) {
        if self
            .sidebar
            .rows
            .get(index)
            .is_some_and(|row| row.item == SidebarItem::Header)
        {
            index = (index + 1).min(self.sidebar.rows.len().saturating_sub(1));
        }
        self.sidebar
            .list_state
            .select((!self.sidebar.rows.is_empty()).then_some(index));
        self.sync_sidebar_filter();
    }

    fn apply_sidebar_filter(&mut self) {
        if self.sync_sidebar_filter() {
            self.focus = Pane::List;
        }
    }

    fn sync_sidebar_filter(&mut self) -> bool {
        let item = self.sidebar.selected().map(|row| row.item.clone());
        match item {
            Some(SidebarItem::All) => self.filter = Filter::default(),
            Some(SidebarItem::Folder(folder)) => {
                self.filter.folder = Some(folder);
                self.filter.tag = None;
            }
            Some(SidebarItem::Tag(tag)) => {
                self.filter.tag = Some(tag);
                self.filter.folder = None;
            }
            _ => return false,
        }
        self.refresh_visible();
        true
    }

    fn toggle_sidebar_folder(&mut self) {
        let folder = self.sidebar.selected().and_then(|row| match &row.item {
            SidebarItem::Folder(folder) if row.has_children => Some(folder.clone()),
            _ => None,
        });
        if let Some(folder) = folder {
            if !self.sidebar.expanded.remove(&folder) {
                self.sidebar.expanded.insert(folder);
            }
            sidebar::rebuild(&mut self.sidebar, &self.catalog);
            self.sync_sidebar_filter();
        } else {
            self.apply_sidebar_filter();
        }
    }

    fn move_list(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let index = (current as isize + delta).clamp(0, self.visible.len() as isize - 1) as usize;
        self.select_list(index);
    }

    fn select_list(&mut self, index: usize) {
        if let Some(row) = self.visible.get(index) {
            self.list_state.select(Some(index));
            self.selected_id = Some(row.snippet_id);
            self.fragment_index = 0;
            self.preview_scroll = 0;
            self.preview.invalidate();
        }
    }

    fn previous_fragment(&mut self) {
        self.fragment_index = self.fragment_index.saturating_sub(1);
        self.preview_scroll = 0;
        self.preview.invalidate();
    }

    fn next_fragment(&mut self) {
        let count = self
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        if self.fragment_index + 1 < count {
            self.fragment_index += 1;
            self.preview_scroll = 0;
            self.preview.invalidate();
        }
    }

    fn clamp_fragment(&mut self) {
        let count = self
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        self.fragment_index = self.fragment_index.min(count.saturating_sub(1));
    }

    fn edit_effect(&mut self) -> Vec<Effect> {
        let Some(snippet) = self.selected_snippet() else {
            return Vec::new();
        };
        if snippet.locked {
            self.set_status(
                "snippet is locked; use the CLI with --force",
                StatusLevel::Error,
            );
            return Vec::new();
        }
        let Some(fragment) = snippet.loaded_fragments.get(self.fragment_index) else {
            return Vec::new();
        };
        let suffix = std::path::Path::new(&fragment.file)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt")
            .to_owned();
        vec![Effect::SpawnEditor(EditRequest {
            snippet_id: snippet.id,
            fragment_id: fragment.id,
            expected: snippet.fingerprint.clone(),
            original: fragment.content.clone(),
            edited: None,
            suffix,
        })]
    }

    fn copy_content_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .and_then(|snippet| snippet.loaded_fragments.get(self.fragment_index))
            .map(|fragment| Effect::CopyToClipboard {
                text: fragment.content.clone(),
                label: "fragment".to_owned(),
            })
            .into_iter()
            .collect()
    }

    fn copy_id_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .map(|snippet| Effect::CopyToClipboard {
                text: snippet.id.to_string(),
                label: "snippet UUID".to_owned(),
            })
            .into_iter()
            .collect()
    }
}
