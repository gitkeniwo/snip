use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::service::restore_snippet;

use super::super::modal::{ConfirmModal, Modal, ModalAction};
use super::super::state::StatusLevel;
use super::types::{App, Effect};

impl App {
    pub(super) fn open_trash(&mut self) {
        match self.trash.open(&self.library) {
            Ok(()) => self.status = None,
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(super) fn handle_trash_key(&mut self, key: KeyEvent) -> Vec<Effect> {
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
}
