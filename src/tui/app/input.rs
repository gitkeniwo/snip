use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::super::layout::{contains, inner};
use super::super::selection::SelectionPoint;
use super::super::state::{Filter, Pane, SidebarItem, StatusLevel};
use super::types::{App, Effect};

impl App {
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
            // Kept as two arms on purpose: a guard on `F(5) | Char('r')` would apply
            // to both alternatives and quietly require Ctrl-F5. Plain `r` must still
            // fall through to rename below, so only the Char arm carries the guard.
            KeyCode::F(5) => self.rescan_now(),
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.rescan_now()
            }
            KeyCode::Char('s') => {
                self.sort = self.sort.next();
                self.refresh_visible();
            }
            KeyCode::Char('e') if self.focus != Pane::Sidebar => return self.edit_effect(),
            KeyCode::Char('v') if self.focus != Pane::Sidebar => return self.open_vscode_effect(),
            KeyCode::Char('E') if self.focus != Pane::Sidebar => return self.edit_note_effect(),
            KeyCode::Char('R') if self.focus != Pane::Sidebar => return self.edit_readme_effect(),
            KeyCode::Char('n') => self.open_new_for_context(),
            KeyCode::Char('d') => self.open_delete_for_context(),
            KeyCode::Char('r') => self.open_rename_for_context(),
            KeyCode::Char('m') => self.open_move_for_context(),
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
}
