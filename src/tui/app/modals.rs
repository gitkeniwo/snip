use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::Snippet;
use crate::error::{Result, SnipError};
use crate::service::{
    CreateOptions, EditOptions, create_folder, create_snippet, delete_folder, delete_snippet,
    delete_tag, edit_snippet, move_folder, purge_snippet, rename_tag,
};

use super::super::editor::{EditRequest, EditTarget};
use super::super::modal::{ConfirmModal, InputModal, Modal, ModalAction, PickerItem, PickerModal};
use super::super::state::{Pane, SidebarItem, StatusLevel};
use super::types::{App, Effect};

impl App {
    pub(super) fn handle_modal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
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
                    // The filter is a text field, so `j`/`k` must stay typable: folder
                    // names like "Docker" would otherwise be unreachable. Navigate with
                    // the arrows or Ctrl-n/Ctrl-p instead.
                    KeyCode::Down => {
                        picker.selected = picker
                            .selected
                            .saturating_add(1)
                            .min(picker.filtered().len().saturating_sub(1));
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.selected = picker
                            .selected
                            .saturating_add(1)
                            .min(picker.filtered().len().saturating_sub(1));
                    }
                    KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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

    pub(super) fn submit_modal(&mut self) -> Vec<Effect> {
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

    pub(super) fn perform_modal_action(
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
                edit_snippet(
                    &self.library,
                    &id.to_string(),
                    &EditOptions {
                        folder: Some(input()?.to_owned()),
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
                let preferred = self
                    .filter
                    .folder
                    .as_deref()
                    .or(self.default_folder.as_deref())
                    .unwrap_or_default();
                let mut picker = PickerModal::new(
                    "Create in folder",
                    self.folder_picker_items(),
                    ModalAction::CreateFolder {
                        title: title.to_owned(),
                    },
                );
                picker.selected = picker
                    .items
                    .iter()
                    .position(|item| item.value == preferred)
                    .unwrap_or(0);
                self.modal = Some(Modal::Picker(picker));
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::CreateFolder { title } => {
                self.modal = Some(Modal::Input(InputModal::new(
                    "Language",
                    self.default_language.clone(),
                    ModalAction::CreateLanguage {
                        title,
                        folder: input()?.to_owned(),
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
            // Mirrors `snip folder rename`: the new name is a single path component and
            // the folder keeps its parent. Reparenting is `MoveFolder` / `snip folder move`.
            ModalAction::RenameFolder { path } => {
                let name = input()?;
                if name.is_empty() {
                    return Err(SnipError::usage("folder name cannot be empty"));
                }
                if std::path::Path::new(name).components().count() != 1 {
                    return Err(SnipError::usage(
                        "new folder name must be one path component",
                    ));
                }
                let target = std::path::Path::new(&path)
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(""))
                    .join(name);
                move_folder(&self.library, &path, &target.to_string_lossy())?;
                "folder renamed".to_owned()
            }
            // Mirrors `snip folder move`: the picked destination becomes the new parent.
            ModalAction::MoveFolder { path } => {
                let parent = input()?;
                let name = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| SnipError::usage("folder has no name"))?;
                let target = if parent.is_empty() {
                    name.to_owned()
                } else {
                    format!("{parent}/{name}")
                };
                if target == path {
                    return Err(SnipError::usage("folder is already in that location"));
                }
                move_folder(&self.library, &path, &target)?;
                "folder moved".to_owned()
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

    pub(super) fn open_new_for_context(&mut self) {
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
            "Create folder",
            value,
            ModalAction::CreateFolderUnder {
                parent: parent.unwrap_or_default(),
            },
        )));
    }

    pub(super) fn open_delete_for_context(&mut self) {
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

    pub(super) fn open_rename_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let selected = self.sidebar.selected().cloned();
            match selected.map(|row| row.item) {
                Some(SidebarItem::Folder(path)) => {
                    // Like `snip folder rename`, this edits the folder name only; the
                    // parent path is fixed. Use `m` to reparent.
                    let name = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or(&path)
                        .to_owned();
                    self.modal = Some(Modal::Input(InputModal::new(
                        "Rename folder",
                        name,
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

    /// Destination rows for every folder picker: the library root shown under the same
    /// `Uncategorized` label the CLI prints, then each folder path.
    pub(super) fn folder_picker_items(&self) -> Vec<PickerItem> {
        let mut items = vec![PickerItem::new(crate::domain::UNCATEGORIZED, "")];
        items.extend(self.catalog.folders.iter().map(PickerItem::plain));
        items
    }

    pub(super) fn open_move_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let Some(SidebarItem::Folder(path)) = self.sidebar.selected().map(|row| &row.item)
            else {
                return;
            };
            let path = path.clone();
            // A folder cannot move inside itself, and the root is spelled `Uncategorized`
            // to match `snip list`.
            let items = self
                .folder_picker_items()
                .into_iter()
                .filter(|item| item.value != path && !item.value.starts_with(&format!("{path}/")))
                .collect::<Vec<_>>();
            self.modal = Some(Modal::Picker(PickerModal::new(
                "Move folder into",
                items,
                ModalAction::MoveFolder { path },
            )));
            return;
        }
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Picker(PickerModal::new(
            "Move to folder",
            self.folder_picker_items(),
            ModalAction::MoveSnippet { id: snippet.id },
        )));
    }

    pub(super) fn open_edit_tags(&mut self) {
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Tags",
            snippet.tags.join(", "),
            ModalAction::EditTags { id: snippet.id },
        )));
    }

    pub(super) fn toggle_pin(&mut self) {
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

    pub(super) fn toggle_lock(&mut self) {
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

    pub(super) fn finish_direct_mutation<T>(&mut self, result: Result<T>) {
        match result {
            Ok(_) => match self.rescan() {
                Ok(()) => self.set_status("snippet updated", StatusLevel::Info),
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(super) fn mutable_selected(&mut self) -> Option<Snippet> {
        let snippet = self.selected_snippet()?.clone();
        if snippet.locked {
            self.set_status("snippet is locked", StatusLevel::Error);
            None
        } else {
            Some(snippet)
        }
    }
}
