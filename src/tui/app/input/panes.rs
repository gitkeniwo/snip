use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::super::layout::{contains, inner};
use super::super::super::state::{Filter, Pane, SidebarItem, StatusLevel};
use super::super::types::App;

impl App {
    pub(super) fn handle_pane_key(&mut self, key: KeyEvent) {
        match self.focus {
            Pane::Sidebar => self.handle_sidebar_key(key),
            Pane::List => self.handle_list_key(key),
            Pane::Preview => self.handle_preview_key(key),
        }
    }

    pub(super) fn handle_sidebar_key(&mut self, key: KeyEvent) {
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

    pub(super) fn handle_list_key(&mut self, key: KeyEvent) {
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

    pub(super) fn handle_preview_key(&mut self, key: KeyEvent) {
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
            KeyCode::Char('{') => crate::tui::preview::jump_paragraph(self, false),
            KeyCode::Char('}') => crate::tui::preview::jump_paragraph(self, true),
            _ => {}
        }
    }

    pub(super) fn drill_back(&mut self) {
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

    pub(super) fn drill_forward(&mut self) {
        match self.focus {
            Pane::Sidebar => self.apply_sidebar_filter(),
            Pane::List => self.focus = Pane::Preview,
            Pane::Preview => {}
        }
    }

    pub(in super::super) fn toggle_density(&mut self) {
        self.density = self.density.next();
        let mut config = match crate::config::AppConfig::load() {
            Ok(config) => config,
            Err(error) => {
                self.set_status(
                    format!("density changed for this session: {error}"),
                    StatusLevel::Error,
                );
                return;
            }
        };
        config
            .tui
            .get_or_insert_with(crate::config::TuiConfig::default)
            .density = self.density;
        match config.save() {
            Ok(()) => self.set_status(
                format!("list density: {}", self.density.label()),
                StatusLevel::Info,
            ),
            Err(error) => self.set_status(
                format!("density changed for this session: {error}"),
                StatusLevel::Error,
            ),
        }
    }

    pub(super) fn click_at(&mut self, column: u16, row: u16) {
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
            let index =
                self.list_state.offset() + ((row - content.y) / self.density.row_height()) as usize;
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

    pub(super) fn scroll_at(&mut self, column: u16, row: u16, direction: isize) {
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

    pub(super) fn move_sidebar(&mut self, delta: isize) {
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

    pub(super) fn select_sidebar(&mut self, mut index: usize) {
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

    pub(super) fn apply_sidebar_filter(&mut self) {
        if self.sync_sidebar_filter() {
            self.focus = Pane::List;
        }
    }

    pub(super) fn sync_sidebar_filter(&mut self) -> bool {
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

    pub(super) fn toggle_sidebar_folder(&mut self) {
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

    pub(super) fn move_list(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let index = (current as isize + delta).clamp(0, self.visible.len() as isize - 1) as usize;
        self.select_list(index);
    }

    pub(super) fn select_list(&mut self, index: usize) {
        if let Some(row) = self.visible.get(index) {
            self.list_state.select(Some(index));
            self.selected_id = Some(row.snippet_id);
            self.fragment_index = 0;
            self.preview_scroll = 0;
            self.preview.invalidate();
        }
    }

    pub(super) fn previous_fragment(&mut self) {
        self.fragment_index = self.fragment_index.saturating_sub(1);
        self.preview_scroll = 0;
        self.preview.invalidate();
    }

    pub(super) fn next_fragment(&mut self) {
        let count = self
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        if self.fragment_index + 1 < count {
            self.fragment_index += 1;
            self.preview_scroll = 0;
            self.preview.invalidate();
        }
    }

    pub(super) fn select_fragment(&mut self, index: usize) {
        let count = self
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        if count > 0 {
            let target = index.min(count.saturating_sub(1));
            if self.fragment_index != target {
                self.fragment_index = target;
                self.preview_scroll = 0;
                self.preview.invalidate();
            }
        }
    }
}
