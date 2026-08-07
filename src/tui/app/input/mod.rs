pub(crate) mod gist;
mod git;
mod overlay;
mod panes;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::super::layout::contains;
use super::super::selection::SelectionPoint;
use super::super::state::Pane;
use super::types::{App, Effect};
use crate::keys::{Chord, Mode};

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let Some(chord) = Chord::from_event(key) else {
            return Vec::new();
        };
        if chord.code() == KeyCode::Char('c') && chord.modifiers() == KeyModifiers::CONTROL {
            return self.run_command(crate::tui::command::CommandId::AppQuit);
        }
        if self.modal.is_some() {
            return self.handle_modal_key(key);
        }
        if self.palette.open {
            return self.handle_palette_key(key);
        }
        let stack = Mode::stack(self);
        if let Some(id) = self.keymap.resolve(&stack, chord) {
            return self.run_command(id);
        }
        // Mode::Search binds nothing itself, so anything the keymap did not claim
        // is text for the query editor. The digit fallback must not see it.
        if self.search.active {
            return self.handle_search(key);
        }

        match chord.code() {
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
                if chord.modifiers().is_empty() =>
            {
                let KeyCode::Char(c) = chord.code() else {
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
            _ => {}
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

#[cfg(test)]
mod tests;
