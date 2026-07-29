use crate::tui::app::types::App;
use crate::tui::modal::{ConfirmModal, InputModal, Modal, ModalAction, PickerModal};
use crate::tui::state::{Pane, SidebarItem, StatusLevel};

impl App {
    pub(in super::super) fn open_new_for_context(&mut self) {
        if self.focus != Pane::Sidebar {
            let _ = self.run_command(crate::tui::command::CommandId::SnippetNew);
            return;
        }
        let _ = self.run_command(crate::tui::command::CommandId::FolderNew);
    }

    pub(in super::super) fn open_new_snippet(&mut self) {
        self.modal = Some(Modal::Input(InputModal::new(
            "Title",
            "",
            ModalAction::CreateTitle,
        )));
    }

    pub(in super::super) fn open_new_folder(&mut self) {
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

    pub(in super::super) fn open_delete_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let selected = self.sidebar.selected().cloned();
            match selected.map(|row| row.item) {
                Some(SidebarItem::Folder(_)) => {
                    let _ = self.run_command(crate::tui::command::CommandId::FolderDelete);
                }
                Some(SidebarItem::Tag(_)) => {
                    let _ = self.run_command(crate::tui::command::CommandId::TagDelete);
                }
                _ => {}
            }
            return;
        }
        let _ = self.run_command(crate::tui::command::CommandId::SnippetMoveToTrash);
    }

    pub(in super::super) fn open_delete_snippet(&mut self) {
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

    pub(in super::super) fn open_delete_folder(&mut self) {
        let Some(SidebarItem::Folder(path)) = self.sidebar.selected().map(|row| &row.item) else {
            return;
        };
        let path = path.clone();
        self.modal = Some(Modal::Confirm(ConfirmModal::new(
            "Delete folder?",
            format!("Delete empty folder {path:?}?"),
            ModalAction::DeleteFolder { path },
            true,
        )));
    }

    pub(in super::super) fn open_delete_tag(&mut self) {
        let Some(SidebarItem::Tag(tag)) = self.sidebar.selected().map(|row| &row.item) else {
            return;
        };
        let tag = tag.clone();
        let count = self.sidebar.selected().map_or(0, |row| row.count);
        self.modal = Some(Modal::Confirm(ConfirmModal::new(
            "Delete tag?",
            format!("Remove #{tag} from {count} snippets?"),
            ModalAction::DeleteTag { tag },
            true,
        )));
    }

    pub(in super::super) fn open_rename_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let selected = self.sidebar.selected().cloned();
            match selected.map(|row| row.item) {
                Some(SidebarItem::Folder(_)) => {
                    let _ = self.run_command(crate::tui::command::CommandId::FolderRename);
                }
                Some(SidebarItem::Tag(_)) => {
                    let _ = self.run_command(crate::tui::command::CommandId::TagRename);
                }
                _ => {}
            }
            return;
        }
        let _ = self.run_command(crate::tui::command::CommandId::SnippetRename);
    }

    pub(in super::super) fn open_rename_snippet(&mut self) {
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Rename",
            snippet.title.clone(),
            ModalAction::RenameSnippet { id: snippet.id },
        )));
    }

    pub(in super::super) fn open_rename_folder(&mut self) {
        let Some(SidebarItem::Folder(path)) = self.sidebar.selected().map(|row| &row.item) else {
            return;
        };
        let path = path.clone();
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

    pub(in super::super) fn open_rename_tag(&mut self) {
        let Some(SidebarItem::Tag(tag)) = self.sidebar.selected().map(|row| &row.item) else {
            return;
        };
        let tag = tag.clone();
        self.modal = Some(Modal::Input(InputModal::new(
            "Rename tag",
            tag.clone(),
            ModalAction::RenameTag { tag },
        )));
    }

    pub(in super::super) fn open_move_for_context(&mut self) {
        if self.focus == Pane::Sidebar {
            let _ = self.run_command(crate::tui::command::CommandId::FolderMove);
            return;
        }
        let _ = self.run_command(crate::tui::command::CommandId::SnippetMove);
    }

    pub(in super::super) fn open_move_snippet(&mut self) {
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Picker(PickerModal::new(
            "Move to folder",
            self.folder_picker_items(),
            ModalAction::MoveSnippet { id: snippet.id },
        )));
    }

    pub(in super::super) fn open_move_folder(&mut self) {
        let Some(SidebarItem::Folder(path)) = self.sidebar.selected().map(|row| &row.item) else {
            return;
        };
        let path = path.clone();
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
    }

    pub(in super::super) fn open_edit_tags(&mut self) {
        let Some(snippet) = self.mutable_selected() else {
            return;
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Tags",
            snippet.tags.join(", "),
            ModalAction::EditTags { id: snippet.id },
        )));
    }

    pub(in super::super) fn open_edit_language(&mut self) {
        let fragment_index = self.fragment_index;
        let Some((id, current)) = self.mutable_selected().and_then(|snippet| {
            snippet
                .loaded_fragments
                .get(fragment_index)
                .map(|fragment| (snippet.id, fragment.language.clone()))
        }) else {
            return;
        };
        let mut picker = PickerModal::new(
            "Language",
            self.language_picker_items(),
            ModalAction::EditLanguage { id, fragment_index },
        )
        .allow_custom()
        .with_current_value(current.clone());
        if let Some(language) = crate::language::info(&current) {
            picker.select_value(language.aliases[0]);
        } else {
            picker.set_filter(current);
        }
        self.modal = Some(Modal::Picker(picker));
    }
}
