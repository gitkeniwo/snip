pub(crate) mod gist;
mod git;
mod overlay;
mod panes;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::super::command::CommandId;
use super::super::layout::contains;
use super::super::selection::SelectionPoint;
use super::super::state::{Filter, Pane, StatusLevel};
use super::types::{App, Effect};

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return self.run_command(CommandId::AppQuit);
        }
        if self.modal.is_some() {
            return self.handle_modal_key(key);
        }
        if self.palette.open {
            return self.handle_palette_key(key);
        }
        if self.git.open {
            return self.handle_git_key(key);
        }
        if self.gist.open {
            return self.handle_gist_key(key);
        }
        if is_ctrl_g(key) {
            if matches!(
                self.git.unavailable.as_ref(),
                Some(crate::git::Unavailable::BinaryMissing)
            ) {
                self.set_status("git not found in PATH", StatusLevel::Error);
            } else {
                return self.run_command(CommandId::GitOpenConsole);
            }
            return Vec::new();
        }
        if is_ctrl_s(key) {
            return self.run_command(CommandId::GistOpenPanel);
        }
        if self.search.active {
            return self.handle_search(key);
        }
        if self.show_help {
            return self.handle_help_key(key);
        }
        if self.trash.open {
            if is_palette_trigger(key) {
                return self.run_command(CommandId::PaletteOpen);
            }
            // The trash occupies the list pane rather than covering everything,
            // so its keys apply only when that pane has focus; the sidebar keeps
            // its own, which is what a popup never allowed.
            if self.focus == Pane::List {
                return self.handle_trash_key(key);
            }
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('T')) {
                self.leave_trash();
                return Vec::new();
            }
        }
        if self.fragment_grab.is_some() {
            return self.handle_fragment_grab_key(key);
        }
        match key.code {
            _ if is_palette_trigger(key) => return self.run_command(CommandId::PaletteOpen),
            KeyCode::Char('q') => return self.run_command(CommandId::AppQuit),
            KeyCode::Tab => return self.run_command(CommandId::PaneNext),
            KeyCode::BackTab => return self.run_command(CommandId::PanePrevious),
            KeyCode::Char('h') | KeyCode::Left => return self.run_command(CommandId::PaneBack),
            KeyCode::Char('l') | KeyCode::Right => {
                return self.run_command(CommandId::PaneForward);
            }
            KeyCode::Char('/') => return self.run_command(CommandId::LibrarySearch),
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
            KeyCode::Char('?') => return self.run_command(CommandId::ViewToggleHelp),
            // Kept as two arms on purpose: a guard on `F(5) | Char('r')` would apply
            // to both alternatives and quietly require Ctrl-F5. Plain `r` must still
            // fall through to rename below, so only the Char arm carries the guard.
            KeyCode::F(5) => return self.run_command(CommandId::LibraryRescan),
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::LibraryRescan);
            }
            KeyCode::Char('s') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::ViewCycleSort);
            }
            KeyCode::Char('z') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::ViewToggleDensity);
            }
            KeyCode::Char('e')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetEditContent);
            }
            KeyCode::Char('v')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetOpenVsCode);
            }
            KeyCode::Char('E')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetEditNote);
            }
            KeyCode::Char('R')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetEditReadme);
            }
            KeyCode::Char('n') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_new_for_context()
            }
            KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_delete_for_context()
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_rename_for_context()
            }
            KeyCode::Char('m') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_move_for_context()
            }
            KeyCode::Char('t')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetEditTags);
            }
            KeyCode::Char('f')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetEditLanguage);
            }
            KeyCode::Char('P')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetTogglePin);
            }
            KeyCode::Char('L')
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.focus != Pane::Sidebar =>
            {
                return self.run_command(CommandId::SnippetToggleLock);
            }
            KeyCode::Char('N') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::ViewToggleLineNumbers);
            }
            KeyCode::Char('T') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::LibraryOpenTrash);
            }
            KeyCode::Char('y') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::CopyContent);
            }
            KeyCode::Char('Y') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::CopySnippetId);
            }
            KeyCode::Char('p') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.run_command(CommandId::CopyManagedPath);
            }
            KeyCode::Char('[') => return self.run_command(CommandId::PreviewPreviousItem),
            KeyCode::Char(']') => return self.run_command(CommandId::PreviewNextItem),
            KeyCode::Char('{') => {
                return self.run_command(CommandId::PreviewPreviousParagraph);
            }
            KeyCode::Char('}') => return self.run_command(CommandId::PreviewNextParagraph),
            KeyCode::Char('1')
            | KeyCode::Char('2')
            | KeyCode::Char('3')
            | KeyCode::Char('4')
            | KeyCode::Char('5')
            | KeyCode::Char('6')
            | KeyCode::Char('7')
            | KeyCode::Char('8')
            | KeyCode::Char('9')
            | KeyCode::Char('0')
                if !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let KeyCode::Char(c) = key.code else {
                    unreachable!()
                };
                let index = if c == '0' {
                    9
                } else {
                    (c as usize) - ('1' as usize)
                };
                match self.focus {
                    Pane::Sidebar => self.select_sidebar(index),
                    Pane::List => self.select_list(index),
                    // Numbered keys address fragments; the README has no number.
                    Pane::Preview => {
                        self.select_fragment(crate::tui::preview::PreviewTarget::Fragment(index))
                    }
                }
            }
            _ => return self.handle_pane_key(key),
        }
        Vec::new()
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) -> Vec<Effect> {
        if self.modal.is_some()
            || self.palette.open
            || self.git.open
            || self.gist.open
            || self.search.active
            || self.fragment_grab.is_some()
        {
            return Vec::new();
        }
        if self.show_help {
            match event.kind {
                MouseEventKind::ScrollUp => self.help_scroll = self.help_scroll.saturating_sub(3),
                MouseEventKind::ScrollDown => self.help_scroll = self.help_scroll.saturating_add(3),
                _ => {}
            }
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
}

pub(super) fn is_ctrl_g(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL)
}

pub(super) fn is_ctrl_s(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL)
}

pub(super) fn is_palette_trigger(key: KeyEvent) -> bool {
    (key.code == KeyCode::Char(':') && !key.modifiers.contains(KeyModifiers::CONTROL))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P')))
}

#[cfg(test)]
mod tests;
