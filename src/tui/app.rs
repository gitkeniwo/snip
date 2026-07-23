use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::widgets::ListState;
use uuid::Uuid;

use crate::config::{AppConfig, TuiIconSetting, TuiSortSetting, TuiThemeSetting};
use crate::domain::{CatalogSnapshot, Snippet};
use crate::error::{Result, SnipError};
use crate::filesystem::Library;
use crate::search::{MemoryIndex, SearchIndex};
use crate::service::{
    CreateOptions, EditOptions, create_folder, create_snippet, delete_folder, delete_snippet,
    delete_tag, edit_snippet, move_folder, purge_snippet, rename_tag, restore_snippet,
    trash_entries,
};

use super::editor::{EditOutcome, EditRequest, EditTarget};
use super::highlight::Highlighter;
use super::icons::IconMode;
use super::layout::{LayoutRects, contains, inner};
use super::modal::{ConfirmModal, InputModal, Modal, ModalAction, PickerModal};
use super::preview::PreviewCache;
use super::selection::{PreviewSelection, SelectionPoint};
use super::sidebar;
use super::state::{
    Filter, Pane, SearchState, SidebarItem, SidebarState, SortMode, StatusLevel, StatusMessage,
    VisibleRow,
};
use super::theme::TuiTheme;
use super::trash::TrashState;

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
    pub show_help: bool,
    pub default_language: String,
    pub default_folder: Option<String>,
    pub default_tags: Vec<String>,
    last_click: Option<(usize, Instant)>,
}

impl App {
    pub fn new(library: Library, config: &AppConfig) -> Result<Self> {
        let catalog = library.scan()?;
        let index = MemoryIndex::new(catalog.clone());
        let tui = config.tui.clone().unwrap_or_default();
        let theme_overrides = tui
            .extra
            .get("colors")
            .and_then(toml::Value::as_table)
            .cloned()
            .unwrap_or_default();
        let theme = TuiTheme::resolve(tui.theme).with_overrides(&theme_overrides);
        let sort = match tui.sort {
            TuiSortSetting::Manual => SortMode::Manual,
            TuiSortSetting::Title => SortMode::Title,
            TuiSortSetting::Modified => SortMode::Modified,
            TuiSortSetting::Created => SortMode::Created,
        };
        let icon_mode = match tui.icons {
            TuiIconSetting::Ascii => IconMode::Ascii,
            TuiIconSetting::Nerd => IconMode::Nerd,
        }
        .effective();
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
            show_line_numbers: true,
            sort,
            layout: LayoutRects::default(),
            preview: PreviewCache::default(),
            preview_selection: PreviewSelection::default(),
            highlighter: Highlighter::new(theme)?,
            theme,
            theme_setting: tui.theme,
            theme_overrides,
            icon_mode,
            theme_checked_at: Instant::now(),
            status: None,
            modal: None,
            trash: TrashState::default(),
            should_quit: false,
            editor_cmd: config.editor.clone(),
            show_help: false,
            default_language: config
                .default_language
                .clone()
                .unwrap_or_else(|| "text".to_owned()),
            default_folder: config.default_folder.clone(),
            default_tags: config.default_tags.clone(),
            last_click: None,
        };
        let trash_count = trash_entries(&app.library).map_or(0, |entries| entries.len());
        sidebar::rebuild(&mut app.sidebar, &app.catalog, trash_count);
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
        if self.modal.is_none() && self.status.as_ref().is_some_and(StatusMessage::expired) {
            self.status = None;
        }
    }

    pub fn tick_theme(&mut self) -> Result<()> {
        if self.theme_setting != TuiThemeSetting::Auto {
            return Ok(());
        }
        if self.theme_checked_at.elapsed() < Duration::from_secs(5) {
            return Ok(());
        }
        self.theme_checked_at = Instant::now();
        let theme = TuiTheme::resolve(self.theme_setting).with_overrides(&self.theme_overrides);
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
        self.rebuild_sidebar();
        self.refresh_visible();
        if self.trash.open {
            self.trash.reload(&self.library)?;
        }
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
            match self.sort {
                SortMode::Manual => snippets.sort_by_key(|snippet| !snippet.pinned),
                SortMode::Title => snippets.sort_by(|left, right| {
                    (!left.pinned)
                        .cmp(&(!right.pinned))
                        .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
                }),
                SortMode::Modified => snippets.sort_by(|left, right| {
                    (!left.pinned)
                        .cmp(&(!right.pinned))
                        .then_with(|| compare_optional_desc(&left.modified_at, &right.modified_at))
                }),
                // `created_at` is emitted in RFC3339 UTC form by snip, so lexical
                // ordering is chronological. Imported mixed offsets may differ slightly.
                SortMode::Created => snippets.sort_by(|left, right| {
                    (!left.pinned)
                        .cmp(&(!right.pinned))
                        .then_with(|| right.created_at.cmp(&left.created_at))
                }),
            }
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
        if self.modal.is_some() {
            return self.handle_modal_key(key);
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
        if self.trash.open {
            return self.handle_trash_key(key);
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Char('h') | KeyCode::Left => self.drill_back(),
            KeyCode::Char('l') | KeyCode::Right => self.drill_forward(),
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
            KeyCode::F(5) | KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match self.rescan() {
                    Ok(()) => self.set_status("library refreshed", StatusLevel::Info),
                    Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
                }
            }
            KeyCode::Char('s') => {
                self.sort = self.sort.next();
                self.refresh_visible();
            }
            KeyCode::Char('e') if self.focus != Pane::Sidebar => return self.edit_effect(),
            KeyCode::Char('E') if self.focus != Pane::Sidebar => return self.edit_note_effect(),
            KeyCode::Char('R') if self.focus != Pane::Sidebar => return self.edit_readme_effect(),
            KeyCode::Char('n') => self.open_new_for_context(),
            KeyCode::Char('d') => self.open_delete_for_context(),
            KeyCode::Char('r') => self.open_rename_for_context(),
            KeyCode::Char('m') if self.focus != Pane::Sidebar => self.open_move_snippet(),
            KeyCode::Char('t') if self.focus != Pane::Sidebar => self.open_edit_tags(),
            KeyCode::Char('p') if self.focus != Pane::Sidebar => self.toggle_pin(),
            KeyCode::Char('L') if self.focus != Pane::Sidebar => self.toggle_lock(),
            KeyCode::Char('N') => {
                self.show_line_numbers = !self.show_line_numbers;
                self.preview_selection.clear();
                self.set_status(
                    if self.show_line_numbers {
                        "line numbers on"
                    } else {
                        "line numbers off"
                    },
                    StatusLevel::Info,
                );
            }
            KeyCode::Char('T') => self.open_trash(),
            KeyCode::Char('y') => return self.copy_content_effect(),
            KeyCode::Char('Y') => return self.copy_id_effect(),
            KeyCode::Char('P') | KeyCode::Char('c') => return self.copy_path_effect(),
            KeyCode::Char('[') => self.previous_fragment(),
            KeyCode::Char(']') => self.next_fragment(),
            _ => self.handle_pane_key(key),
        }
        Vec::new()
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) -> Vec<Effect> {
        if self.modal.is_some() || self.trash.open || self.show_help || self.search.active {
            return Vec::new();
        }
        match event.kind {
            MouseEventKind::ScrollUp => self.scroll_at(event.column, event.row, -1),
            MouseEventKind::ScrollDown => self.scroll_at(event.column, event.row, 1),
            MouseEventKind::Down(MouseButton::Left) => {
                if contains(self.layout.preview_content, event.column, event.row)
                    && let Some(point) =
                        self.preview_selection_point(event.column, event.row, false)
                {
                    self.preview_selection.begin(point);
                    self.focus = Pane::Preview;
                    return Vec::new();
                }
                self.preview_selection.clear();
                self.click_at(event.column, event.row);
            }
            MouseEventKind::Drag(MouseButton::Left) if self.preview_selection.is_dragging() => {
                if let Some(point) = self.preview_selection_point(event.column, event.row, true) {
                    self.preview_selection.update(point);
                }
            }
            MouseEventKind::Up(MouseButton::Left) if self.preview_selection.is_dragging() => {
                if let Some(point) = self.preview_selection_point(event.column, event.row, true)
                    && let Some(text) = self.preview_selection.finish(point)
                {
                    return vec![Effect::CopyToClipboard {
                        text,
                        label: "selection".to_owned(),
                    }];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn preview_selection_point(
        &self,
        column: u16,
        row: u16,
        clamp: bool,
    ) -> Option<SelectionPoint> {
        let area = self.layout.preview_content;
        if area.is_empty() {
            return None;
        }
        if !clamp && !contains(area, column, row) {
            return None;
        }
        let column = column.clamp(area.x, area.right().saturating_sub(1)) - area.x;
        let visible_row = row.clamp(area.y, area.bottom().saturating_sub(1)) - area.y;
        let logical_row = self.preview_scroll as usize + visible_row as usize;
        self.preview_selection.point_at(logical_row, column)
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
                    self.set_status("snippet saved", StatusLevel::Info);
                }
            }
            EditOutcome::Conflict(request) => {
                self.modal = Some(Modal::Confirm(ConfirmModal::new(
                    "Overwrite changed snippet?",
                    "The snippet changed on disk while the editor was open.",
                    ModalAction::ForceEdit(request),
                    true,
                )));
            }
        }
    }

    fn matches_filter(&self, snippet: &Snippet) -> bool {
        if self.filter.uncategorized {
            return snippet.folder.is_empty();
        }
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

    fn handle_modal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let mut submit = false;
        let mut cancel = false;
        if let Some(modal) = self.modal.as_mut() {
            match modal {
                Modal::Input(input) => match key.code {
                    KeyCode::Enter => submit = true,
                    KeyCode::Esc => cancel = true,
                    KeyCode::Left => input.cursor = input.cursor.saturating_sub(1),
                    KeyCode::Right => {
                        input.cursor = (input.cursor + 1).min(input.value.chars().count())
                    }
                    KeyCode::Home => input.cursor = 0,
                    KeyCode::End => input.cursor = input.value.chars().count(),
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Char(value)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        input.insert(value)
                    }
                    _ => {}
                },
                Modal::Confirm(_) => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => submit = true,
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => cancel = true,
                    _ => {}
                },
                Modal::Picker(picker) => match key.code {
                    KeyCode::Enter => submit = true,
                    KeyCode::Esc => cancel = true,
                    KeyCode::Char('j') | KeyCode::Down => {
                        picker.selected = picker
                            .selected
                            .saturating_add(1)
                            .min(picker.filtered().len().saturating_sub(1));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        picker.selected = picker.selected.saturating_sub(1)
                    }
                    KeyCode::Home => picker.selected = 0,
                    KeyCode::End => picker.selected = picker.filtered().len().saturating_sub(1),
                    KeyCode::Backspace => {
                        picker.filter.pop();
                        picker.clamp();
                        picker.error = None;
                    }
                    KeyCode::Char(value)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        picker.filter.push(value);
                        picker.selected = 0;
                        picker.error = None;
                    }
                    _ => {}
                },
            }
        }
        if cancel {
            let force_edit = self
                .modal
                .as_ref()
                .is_some_and(|modal| matches!(modal.action(), ModalAction::ForceEdit(_)));
            self.modal = None;
            if force_edit {
                self.set_status("edited content discarded", StatusLevel::Info);
            }
            return Vec::new();
        }
        if submit {
            return self.submit_modal();
        }
        Vec::new()
    }

    fn submit_modal(&mut self) -> Vec<Effect> {
        let Some(mut modal) = self.modal.take() else {
            return Vec::new();
        };
        let action = modal.action().clone();
        let value = match &modal {
            Modal::Input(input) => Some(input.value.clone()),
            Modal::Confirm(_) => None,
            Modal::Picker(picker) => picker.selected_value(),
        };
        if matches!(modal, Modal::Picker(_)) && value.is_none() {
            modal.set_error("no matching item");
            self.modal = Some(modal);
            return Vec::new();
        }
        match self.perform_modal_action(action, value.as_deref()) {
            Ok((effects, message)) => {
                if !message.is_empty() {
                    self.set_status(message, StatusLevel::Info);
                }
                effects
            }
            Err(error) => {
                modal.set_error(error.to_string());
                self.modal = Some(modal);
                Vec::new()
            }
        }
    }

    fn perform_modal_action(
        &mut self,
        action: ModalAction,
        value: Option<&str>,
    ) -> Result<(Vec<Effect>, String)> {
        let input = || {
            value
                .map(str::trim)
                .ok_or_else(|| SnipError::usage("modal input is unavailable"))
        };
        let message = match action {
            ModalAction::RenameSnippet { id } => {
                edit_snippet(
                    &self.library,
                    &id.to_string(),
                    &EditOptions {
                        title: Some(input()?.to_owned()),
                        ..EditOptions::default()
                    },
                )?;
                "snippet renamed".to_owned()
            }
            ModalAction::MoveSnippet { id } => {
                let folder = input()?;
                edit_snippet(
                    &self.library,
                    &id.to_string(),
                    &EditOptions {
                        folder: Some(if folder == "~" { "" } else { folder }.to_owned()),
                        ..EditOptions::default()
                    },
                )?;
                "snippet moved".to_owned()
            }
            ModalAction::EditTags { id } => {
                let tags = input()?
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                edit_snippet(
                    &self.library,
                    &id.to_string(),
                    &EditOptions {
                        tags: Some(tags),
                        ..EditOptions::default()
                    },
                )?;
                "tags updated".to_owned()
            }
            ModalAction::DeleteSnippet { id } => {
                delete_snippet(&self.library, &id.to_string(), None, false)?;
                "snippet moved to trash".to_owned()
            }
            ModalAction::ForceEdit(request) => {
                return Ok((vec![Effect::ForceSave(request)], String::new()));
            }
            ModalAction::CreateTitle => {
                let title = input()?;
                if title.is_empty() {
                    return Err(SnipError::usage("snippet title cannot be empty"));
                }
                let mut folders = vec!["~".to_owned()];
                folders.extend(self.catalog.folders.iter().cloned());
                let preferred = self
                    .filter
                    .folder
                    .as_ref()
                    .or(self.default_folder.as_ref())
                    .map_or("~", String::as_str);
                let mut picker = PickerModal::new(
                    "Create in folder",
                    folders,
                    ModalAction::CreateFolder {
                        title: title.to_owned(),
                    },
                );
                picker.selected = picker
                    .items
                    .iter()
                    .position(|folder| folder == preferred)
                    .unwrap_or(0);
                self.modal = Some(Modal::Picker(picker));
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::CreateFolder { title } => {
                let folder = input()?;
                self.modal = Some(Modal::Input(InputModal::new(
                    "Language",
                    self.default_language.clone(),
                    ModalAction::CreateLanguage {
                        title,
                        folder: if folder == "~" { "" } else { folder }.to_owned(),
                    },
                )));
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::CreateLanguage { title, folder } => {
                let created = create_snippet(
                    &self.library,
                    &CreateOptions {
                        title,
                        folder: (!folder.is_empty()).then_some(folder),
                        tags: self.default_tags.clone(),
                        language: input()?.to_owned(),
                        content: String::new(),
                        ..CreateOptions::default()
                    },
                )?;
                let fragment = created
                    .loaded_fragments
                    .first()
                    .ok_or_else(|| SnipError::validation("new snippet has no fragment"))?;
                let suffix = std::path::Path::new(&fragment.file)
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("txt")
                    .to_owned();
                let request = EditRequest {
                    snippet_id: created.id,
                    target: EditTarget::Content {
                        fragment_id: fragment.id,
                    },
                    expected: created.fingerprint.clone(),
                    original: fragment.content.clone(),
                    edited: None,
                    suffix,
                };
                self.rescan()?;
                self.selected_id = Some(created.id);
                if let Some(index) = self
                    .visible
                    .iter()
                    .position(|row| row.snippet_id == created.id)
                {
                    self.list_state.select(Some(index));
                }
                self.focus = Pane::List;
                return Ok((
                    vec![Effect::SpawnEditor(request)],
                    "snippet created".to_owned(),
                ));
            }
            ModalAction::CreateFolderUnder { .. } => {
                create_folder(&self.library, input()?)?;
                "folder created".to_owned()
            }
            ModalAction::RenameFolder { path } => {
                move_folder(&self.library, &path, input()?)?;
                "folder renamed".to_owned()
            }
            ModalAction::DeleteFolder { path } => {
                delete_folder(&self.library, &path)?;
                "folder deleted".to_owned()
            }
            ModalAction::RenameTag { tag } => {
                let count = rename_tag(&self.library, &tag, input()?)?;
                format!("tag renamed in {count} snippets")
            }
            ModalAction::DeleteTag { tag } => {
                let count = delete_tag(&self.library, &tag)?;
                format!("tag removed from {count} snippets")
            }
            ModalAction::PurgeSnippet { entry_id } => {
                purge_snippet(&self.library, &entry_id)?;
                self.trash.reload(&self.library)?;
                "trash entry permanently deleted".to_owned()
            }
        };
        self.rescan()?;
        Ok((Vec::new(), message))
    }

    fn open_new_for_context(&mut self) {
        if self.focus != Pane::Sidebar {
            self.modal = Some(Modal::Input(InputModal::new(
                "Title",
                "",
                ModalAction::CreateTitle,
            )));
            return;
        }
        let parent = match self.sidebar.selected().map(|row| &row.item) {
            Some(SidebarItem::Folder(path)) => Some(path.clone()),
            Some(SidebarItem::All) | Some(SidebarItem::Uncategorized) => None,
            _ => {
                self.set_status(
                    "select a folder before creating a subfolder",
                    StatusLevel::Error,
                );
                return;
            }
        };
        let value = parent
            .as_ref()
            .map_or(String::new(), |path| format!("{path}/"));
        self.modal = Some(Modal::Input(InputModal::new(
            "New folder",
            value,
            ModalAction::CreateFolderUnder {
                parent: parent.unwrap_or_default(),
            },
        )));
    }

    fn open_delete_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let selected = self.sidebar.selected().cloned();
            match selected.map(|row| row.item) {
                Some(SidebarItem::Folder(path)) => {
                    self.modal = Some(Modal::Confirm(ConfirmModal::new(
                        "Delete folder?",
                        format!("Delete empty folder {path:?}?"),
                        ModalAction::DeleteFolder { path },
                        true,
                    )));
                }
                Some(SidebarItem::Tag(tag)) => {
                    let count = self.sidebar.selected().map_or(0, |row| row.count);
                    self.modal = Some(Modal::Confirm(ConfirmModal::new(
                        "Delete tag?",
                        format!("Remove #{tag} from {count} snippets?"),
                        ModalAction::DeleteTag { tag },
                        true,
                    )));
                }
                _ => {}
            }
            return;
        }
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Confirm(ConfirmModal::new(
            "Move snippet to trash?",
            format!("Delete {:?}? It can be restored from Trash.", snippet.title),
            ModalAction::DeleteSnippet { id: snippet.id },
            true,
        )));
    }

    fn open_rename_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let selected = self.sidebar.selected().cloned();
            match selected.map(|row| row.item) {
                Some(SidebarItem::Folder(path)) => {
                    self.modal = Some(Modal::Input(InputModal::new(
                        "Rename folder",
                        path.clone(),
                        ModalAction::RenameFolder { path },
                    )));
                }
                Some(SidebarItem::Tag(tag)) => {
                    self.modal = Some(Modal::Input(InputModal::new(
                        "Rename tag",
                        tag.clone(),
                        ModalAction::RenameTag { tag },
                    )));
                }
                _ => {}
            }
            return;
        }
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Rename",
            snippet.title.clone(),
            ModalAction::RenameSnippet { id: snippet.id },
        )));
    }

    fn open_move_snippet(&mut self) {
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        let mut folders = vec!["~".to_owned()];
        folders.extend(self.catalog.folders.iter().cloned());
        self.modal = Some(Modal::Picker(PickerModal::new(
            "Move to folder",
            folders,
            ModalAction::MoveSnippet { id: snippet.id },
        )));
    }

    fn open_edit_tags(&mut self) {
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Tags",
            snippet.tags.join(", "),
            ModalAction::EditTags { id: snippet.id },
        )));
    }

    fn toggle_pin(&mut self) {
        let Some(snippet) = self.selected_snippet().cloned() else {
            return;
        };
        let result = edit_snippet(
            &self.library,
            &snippet.id.to_string(),
            &EditOptions {
                pinned: Some(!snippet.pinned),
                force: snippet.locked,
                ..EditOptions::default()
            },
        );
        self.finish_direct_mutation(result.map(|_| "pin updated"));
    }

    fn toggle_lock(&mut self) {
        let Some(snippet) = self.selected_snippet().cloned() else {
            return;
        };
        let result = edit_snippet(
            &self.library,
            &snippet.id.to_string(),
            &EditOptions {
                locked: Some(!snippet.locked),
                force: snippet.locked,
                ..EditOptions::default()
            },
        );
        self.finish_direct_mutation(result.map(|_| "lock updated"));
    }

    fn finish_direct_mutation<T>(&mut self, result: Result<T>) {
        match result {
            Ok(_) => match self.rescan() {
                Ok(()) => self.set_status("snippet updated", StatusLevel::Info),
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    fn mutable_selected(&mut self) -> Option<Snippet> {
        let snippet = self.selected_snippet()?.clone();
        if snippet.locked {
            self.set_status("snippet is locked", StatusLevel::Error);
            None
        } else {
            Some(snippet)
        }
    }

    fn open_trash(&mut self) {
        match self.trash.open(&self.library) {
            Ok(()) => self.status = None,
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    fn handle_trash_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc | KeyCode::Char('T') => self.trash.open = false,
            KeyCode::Char('j') | KeyCode::Down => self.trash.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.trash.move_selection(-1),
            KeyCode::Char('g') | KeyCode::Home => self.trash.selected = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.trash.selected = self.trash.entries.len().saturating_sub(1)
            }
            KeyCode::Enter | KeyCode::Char('u') => {
                let Some(entry) = self.trash.selected().cloned() else {
                    return Vec::new();
                };
                match restore_snippet(&self.library, &entry.entry_id, None) {
                    Ok(_) => match self.rescan() {
                        Ok(()) => self.set_status("snippet restored", StatusLevel::Info),
                        Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
                    },
                    Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
                }
            }
            KeyCode::Char('x') => {
                let Some(entry) = self.trash.selected().cloned() else {
                    return Vec::new();
                };
                self.modal = Some(Modal::Confirm(ConfirmModal::new(
                    "Permanently delete?",
                    format!("Purge {:?}? This cannot be undone.", entry.title),
                    ModalAction::PurgeSnippet {
                        entry_id: entry.entry_id,
                    },
                    true,
                )));
            }
            _ => {}
        }
        Vec::new()
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

    fn drill_back(&mut self) {
        match self.focus {
            Pane::Preview => self.focus = Pane::List,
            Pane::List => self.focus = Pane::Sidebar,
            Pane::Sidebar => {
                let folder = self.sidebar.selected().and_then(|row| match &row.item {
                    SidebarItem::Folder(folder) if row.expanded => Some(folder.clone()),
                    _ => None,
                });
                if let Some(folder) = folder {
                    self.sidebar.expanded.remove(&folder);
                    self.rebuild_sidebar();
                    self.sync_sidebar_filter();
                }
            }
        }
    }

    fn drill_forward(&mut self) {
        match self.focus {
            Pane::Sidebar => self.apply_sidebar_filter(),
            Pane::List => self.focus = Pane::Preview,
            Pane::Preview => {}
        }
    }

    fn click_at(&mut self, column: u16, row: u16) {
        if contains(self.layout.sidebar, column, row) {
            let content = inner(self.layout.sidebar);
            if !contains(content, column, row) {
                self.focus = Pane::Sidebar;
                return;
            }
            let index = self.sidebar.list_state.offset() + (row - content.y) as usize;
            if index >= self.sidebar.rows.len() {
                return;
            }
            self.sidebar.list_state.select(Some(index));
            self.focus = Pane::Sidebar;
            let fold_column = content
                .x
                .saturating_add(self.sidebar.rows[index].depth.saturating_mul(2) as u16);
            if self.sidebar.rows[index].has_children && column <= fold_column.saturating_add(1) {
                self.toggle_sidebar_folder();
            } else {
                self.sync_sidebar_filter();
            }
            return;
        }
        if contains(self.layout.list, column, row) {
            let content = inner(self.layout.list);
            if !contains(content, column, row) {
                self.focus = Pane::List;
                return;
            }
            let index = self.list_state.offset() + ((row - content.y) / 2) as usize;
            if index >= self.visible.len() {
                return;
            }
            self.select_list(index);
            self.focus = Pane::List;
            let now = Instant::now();
            let double = self.last_click.is_some_and(|(previous, at)| {
                previous == index && now.duration_since(at) < Duration::from_millis(500)
            });
            self.last_click = Some((index, now));
            if double {
                self.focus = Pane::Preview;
            }
            return;
        }
        if contains(self.layout.preview_tabs, column, row) {
            for (index, (start, end)) in self.layout.tab_spans[..self.layout.tab_count]
                .iter()
                .enumerate()
            {
                if column >= *start && column < *end {
                    self.fragment_index = index;
                    self.preview_scroll = 0;
                    self.preview.invalidate();
                    self.focus = Pane::Preview;
                    return;
                }
            }
        }
        if contains(self.layout.preview, column, row) {
            self.focus = Pane::Preview;
        }
    }

    fn scroll_at(&mut self, column: u16, row: u16, direction: isize) {
        if contains(self.layout.sidebar, column, row) {
            self.move_sidebar(direction);
        } else if contains(self.layout.list, column, row) {
            self.move_list(direction);
        } else if contains(self.layout.preview, column, row) {
            if direction < 0 {
                self.preview_scroll = self.preview_scroll.saturating_sub(3);
            } else {
                self.preview_scroll = self.preview_scroll.saturating_add(3);
            }
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
            Some(SidebarItem::All) => {
                self.filter = Filter::default();
            }
            Some(SidebarItem::Uncategorized) => {
                self.filter = Filter {
                    uncategorized: true,
                    folder: None,
                    tag: None,
                };
            }
            Some(SidebarItem::Folder(folder)) => {
                self.filter.uncategorized = false;
                self.filter.folder = Some(folder);
                self.filter.tag = None;
            }
            Some(SidebarItem::Tag(tag)) => {
                self.filter.uncategorized = false;
                self.filter.tag = Some(tag);
                self.filter.folder = None;
            }
            Some(SidebarItem::Trash) => {
                self.open_trash();
                return false;
            }
            _ => return false,
        }
        self.refresh_visible();
        true
    }

    fn rebuild_sidebar(&mut self) {
        let trash_count = trash_entries(&self.library).map_or(0, |entries| entries.len());
        sidebar::rebuild(&mut self.sidebar, &self.catalog, trash_count);
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
            self.rebuild_sidebar();
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
            target: EditTarget::Content {
                fragment_id: fragment.id,
            },
            expected: snippet.fingerprint.clone(),
            original: fragment.content.clone(),
            edited: None,
            suffix,
        })]
    }

    fn edit_note_effect(&mut self) -> Vec<Effect> {
        let Some(snippet) = self.mutable_selected() else {
            return Vec::new();
        };
        let Some(fragment) = snippet.loaded_fragments.get(self.fragment_index) else {
            return Vec::new();
        };
        vec![Effect::SpawnEditor(EditRequest {
            snippet_id: snippet.id,
            target: EditTarget::Note {
                fragment_id: fragment.id,
            },
            expected: snippet.fingerprint.clone(),
            original: fragment.note_content.clone().unwrap_or_default(),
            edited: None,
            suffix: "md".to_owned(),
        })]
    }

    fn edit_readme_effect(&mut self) -> Vec<Effect> {
        let Some(snippet) = self.mutable_selected() else {
            return Vec::new();
        };
        vec![Effect::SpawnEditor(EditRequest {
            snippet_id: snippet.id,
            target: EditTarget::Readme,
            expected: snippet.fingerprint.clone(),
            original: snippet.readme.clone().unwrap_or_default(),
            edited: None,
            suffix: "md".to_owned(),
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

    fn copy_path_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .map(|snippet| {
                let path = snippet
                    .loaded_fragments
                    .get(self.fragment_index)
                    .map(|fragment| &fragment.absolute_path)
                    .unwrap_or(&snippet.package_path);
                Effect::CopyToClipboard {
                    text: path.display().to_string(),
                    label: "snippet path".to_owned(),
                }
            })
            .into_iter()
            .collect()
    }
}

fn compare_optional_desc(left: &Option<String>, right: &Option<String>) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(left),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}
