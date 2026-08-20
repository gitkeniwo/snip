use std::sync::Arc;

use crate::service::restore_snippet;

use super::super::modal::{ConfirmModal, Modal, ModalAction};
use super::super::state::{SidebarItem, StatusLevel};
use super::types::App;

impl App {
    pub(super) fn open_trash(&mut self) {
        match self.trash.open(&self.library) {
            Ok(()) => self.status = None,
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
        // The trash view is tied to the sidebar row, so entering it by any other
        // route (the `T` key, the palette) parks the cursor there too. Sets the
        // index directly: going through the sidebar would recurse back here.
        self.select_sidebar_item(&SidebarItem::Trash);
        self.sync_trash_preview();
    }

    pub(super) fn leave_trash(&mut self) {
        self.trash.open = false;
        self.trash.preview = None;
        self.select_sidebar_item(&SidebarItem::All);
        self.sync_sidebar_filter();
    }

    fn select_sidebar_item(&mut self, item: &SidebarItem) {
        if let Some(index) = self.sidebar.rows.iter().position(|row| &row.item == item) {
            self.sidebar.list_state.select(Some(index));
        }
    }

    /// Loads the selected entry's package so the preview pane can render it.
    /// A trashed package that fails to load simply previews as empty — the
    /// entry is still restorable, and refusing to draw would be worse.
    pub(super) fn sync_trash_preview(&mut self) {
        let entry = self.trash.selected().cloned();
        self.trash.preview = entry.and_then(|entry| {
            let mut snippet = self.library.load_snippet(&entry.package_path).ok()?;
            // The package sits under trash/, so the folder derived from its path
            // is meaningless. Restore the one it will return to.
            snippet.folder = entry
                .original_path
                .strip_prefix("snippets/")
                .unwrap_or(&entry.original_path)
                .rsplit_once('/')
                .map(|(folder, _)| folder.to_owned())
                .unwrap_or_default();
            Some(Arc::new(snippet))
        });
    }

    pub(super) fn restore_selected_trash(&mut self) {
        let Some(entry) = self.trash.selected().cloned() else {
            return;
        };
        match restore_snippet(&self.library, &entry.entry_id, None) {
            Ok(_) => match self.rescan() {
                Ok(()) => self.set_status("snippet restored", StatusLevel::Info),
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(super) fn purge_selected_trash(&mut self) {
        let Some(entry) = self.trash.selected().cloned() else {
            return;
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
}
