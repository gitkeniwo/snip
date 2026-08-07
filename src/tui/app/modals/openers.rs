use crate::service::{FragmentAddOptions, add_fragment};
use crate::tui::app::types::App;
use crate::tui::modal::{ConfirmModal, InputModal, Modal, ModalAction, PickerItem, PickerModal};
use crate::tui::state::{Pane, SidebarItem, StatusLevel};

use super::super::types::FragmentGrab;

impl App {
    pub(crate) fn fragment_context(&self) -> bool {
        self.fragments_expanded && self.focus == Pane::Preview && !self.trash.open
    }

    pub(in super::super) fn open_theme_picker(&mut self) {
        let items = crate::theme::list()
            .into_iter()
            .filter(|summary| summary.appearance == self.theme.appearance)
            .map(|summary| {
                let label = if summary.error.is_some() {
                    format!("{} (invalid)", summary.display_name)
                } else {
                    summary.display_name
                };
                let source_slug = summary
                    .source
                    .as_deref()
                    .and_then(|source| source.strip_prefix("base16:"))
                    .unwrap_or_default()
                    .to_owned();
                PickerItem::with_keywords(label, summary.name.clone(), [summary.name, source_slug])
            })
            .collect();
        let mut picker = PickerModal::new("Color Theme", items, ModalAction::PickTheme)
            .with_current_value(self.theme_name.clone());
        picker.select_value(&self.theme_name);
        self.modal = Some(Modal::Picker(picker));
    }

    pub(in super::super) fn open_new_for_context(&mut self) {
        if self.fragment_context() {
            let _ = self.run_command(crate::tui::command::CommandId::FragmentAdd);
            return;
        }
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
        if self.fragment_context() {
            let _ = self.run_command(crate::tui::command::CommandId::FragmentRemove);
            return;
        }
        if self.focus == Pane::Sidebar {
            let _ = self.run_command(crate::tui::command::CommandId::SidebarDelete);
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
        if self.fragment_context() {
            let _ = self.run_command(crate::tui::command::CommandId::FragmentRename);
            return;
        }
        if self.focus == Pane::Sidebar {
            let _ = self.run_command(crate::tui::command::CommandId::SidebarRename);
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
        if self.fragment_context() {
            let _ = self.run_command(crate::tui::command::CommandId::FragmentReorder);
            return;
        }
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
        // The README is always Markdown, so there is nothing to pick.
        let Some(fragment_index) = self.preview_target.fragment_index() else {
            return;
        };
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

    pub(in super::super) fn open_add_fragment(&mut self) {
        let Some(snippet) = self.selected_snippet().cloned() else {
            return;
        };
        let mut number = snippet.loaded_fragments.len() + 1;
        let title = loop {
            let candidate = format!("Fragment {number}");
            if snippet
                .loaded_fragments
                .iter()
                .all(|fragment| fragment.title != candidate)
            {
                break candidate;
            }
            number += 1;
        };
        let language = self
            .preview_target
            .fragment_index()
            .and_then(|index| snippet.loaded_fragments.get(index))
            .map(|fragment| fragment.language.as_str())
            .filter(|language| !language.is_empty())
            .or_else(|| {
                (!self.default_language.is_empty()).then_some(self.default_language.as_str())
            })
            .unwrap_or("text")
            .to_owned();
        let result = add_fragment(
            &self.library,
            &snippet.id.to_string(),
            &FragmentAddOptions {
                id: None,
                title: title.clone(),
                language,
                source_language: None,
                content: String::new(),
                note: None,
                if_hash: None,
                force: false,
            },
        );
        match result {
            Ok(_) => match self.rescan() {
                Ok(()) => {
                    self.preview_target = crate::tui::preview::PreviewTarget::Fragment(
                        self.selected_snippet().map_or(0, |snippet| {
                            snippet.loaded_fragments.len().saturating_sub(1)
                        }),
                    );
                    self.set_status(format!("{title} added; press e to edit"), StatusLevel::Info);
                }
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(in super::super) fn open_rename_fragment(&mut self) {
        let Some(fragment_index) = self.preview_target.fragment_index() else {
            return;
        };
        let Some((id, title)) = self.selected_snippet().and_then(|snippet| {
            snippet
                .loaded_fragments
                .get(fragment_index)
                .map(|fragment| (snippet.id, fragment.title.clone()))
        }) else {
            return;
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Rename fragment",
            title,
            ModalAction::RenameFragment { id, fragment_index },
        )));
    }

    pub(in super::super) fn open_delete_fragment(&mut self) {
        let Some(fragment_index) = self.preview_target.fragment_index() else {
            return;
        };
        let Some((id, title)) = self.selected_snippet().and_then(|snippet| {
            snippet
                .loaded_fragments
                .get(fragment_index)
                .map(|fragment| (snippet.id, crate::tui::preview::fragment_label(fragment)))
        }) else {
            return;
        };
        self.modal = Some(Modal::Confirm(ConfirmModal::new(
            "Delete fragment?",
            format!("Delete {title:?}? Fragments are not moved to Trash."),
            ModalAction::DeleteFragment { id, fragment_index },
            true,
        )));
    }

    pub(in super::super) fn start_fragment_grab(&mut self) {
        // The README is never a drag source and never a drop position.
        let Some(index) = self.preview_target.fragment_index() else {
            return;
        };
        self.fragment_grab = Some(FragmentGrab {
            origin: index,
            current: index,
        });
        self.set_status(
            "moving fragment; j/k to move, Enter to drop, Esc to cancel",
            StatusLevel::Info,
        );
    }
}
