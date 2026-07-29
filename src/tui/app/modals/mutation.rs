use crate::domain::Snippet;
use crate::error::Result;
use crate::service::{EditOptions, edit_snippet};

use crate::tui::app::types::App;
use crate::tui::state::StatusLevel;

impl App {
    pub(in super::super) fn toggle_pin(&mut self) {
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

    pub(in super::super) fn toggle_lock(&mut self) {
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

    pub(in super::super) fn finish_direct_mutation<T>(&mut self, result: Result<T>) {
        match result {
            Ok(_) => match self.rescan() {
                Ok(()) => self.set_status("snippet updated", StatusLevel::Info),
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(in super::super) fn mutable_selected(&mut self) -> Option<Snippet> {
        let snippet = self.selected_snippet()?.clone();
        if snippet.locked {
            self.set_status("snippet is locked", StatusLevel::Error);
            None
        } else {
            Some(snippet)
        }
    }
}
