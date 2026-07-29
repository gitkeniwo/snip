use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::super::command::CommandId;
use super::super::super::state::Pane;
use super::super::types::{App, Effect};

impl App {
    pub(super) fn handle_palette_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        use KeyCode::*;
        match key.code {
            Esc => self.palette.close(),
            Enter => {
                let id = self.palette.selected_id();
                self.palette.close();
                if let Some(id) = id {
                    return self.run_command(id);
                }
            }
            Up => self.palette.move_selection(-1),
            Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette.move_selection(-1)
            }
            Down => self.palette.move_selection(1),
            Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette.move_selection(1)
            }
            Left => self.palette.input.cursor = self.palette.input.cursor.saturating_sub(1),
            Right => {
                self.palette.input.cursor =
                    (self.palette.input.cursor + 1).min(self.palette.input.value.chars().count())
            }
            Home => self.palette.input.cursor = 0,
            Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette.input.cursor = 0
            }
            End => self.palette.input.cursor = self.palette.input.value.chars().count(),
            Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette.input.cursor = self.palette.input.value.chars().count()
            }
            Backspace => {
                self.palette.input.backspace();
                self.refresh_palette();
            }
            Delete => {
                self.palette.input.delete();
                self.refresh_palette();
            }
            Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette.input.insert(character);
                self.refresh_palette();
            }
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn handle_search(&mut self, key: KeyEvent) -> Vec<Effect> {
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
    pub(super) fn handle_help_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            _ if super::is_palette_trigger(key) => self.open_palette(),
            KeyCode::Char('q') => return self.run_command(CommandId::AppQuit),
            KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
            KeyCode::Char('j') | KeyCode::Down => {
                self.help_scroll = self.help_scroll.saturating_add(1)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.help_scroll = self.help_scroll.saturating_sub(1)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_scroll = self.help_scroll.saturating_add(10)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.help_scroll = self.help_scroll.saturating_sub(10)
            }
            _ => {}
        }
        Vec::new()
    }
}
