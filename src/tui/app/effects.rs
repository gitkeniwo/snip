use super::super::editor::{EditOutcome, EditRequest, EditTarget};
use super::super::modal::{ConfirmModal, Modal, ModalAction};
use super::super::preview::PreviewTarget;
use super::super::state::StatusLevel;
use super::types::{App, Effect};
use crate::external_editor::{EditorTargetKind, editor_dir_for_target, resolve_editor_cwd};

impl App {
    pub fn handle_editor_outcome(&mut self, outcome: EditOutcome) {
        match outcome {
            EditOutcome::Unchanged => {
                self.set_status("editor closed without changes", StatusLevel::Info)
            }
            EditOutcome::Saved => {
                if let Err(error) = self.rescan() {
                    self.set_status(error.to_string(), StatusLevel::Error);
                } else {
                    self.set_status("snippet saved", StatusLevel::Info);
                }
            }
            EditOutcome::Conflict(request) => {
                self.modal = Some(Modal::Confirm(ConfirmModal::new(
                    "Overwrite changed snippet?",
                    "The snippet changed on disk while the editor was open.",
                    ModalAction::ForceEdit(request),
                    true,
                )));
            }
        }
    }

    pub(super) fn edit_effect(&mut self) -> Vec<Effect> {
        if self.preview_target == PreviewTarget::Readme {
            return self.edit_readme_effect();
        }
        let Some(snippet) = self.selected_snippet() else {
            return Vec::new();
        };
        if snippet.locked {
            self.set_status(
                "snippet is locked; use the CLI with --force",
                StatusLevel::Error,
            );
            return Vec::new();
        }
        let Some(fragment) = self
            .preview_target
            .fragment_index()
            .and_then(|index| snippet.loaded_fragments.get(index))
        else {
            return Vec::new();
        };
        let suffix = std::path::Path::new(&fragment.file)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("txt")
            .to_owned();
        let leaf_dir = editor_dir_for_target(
            &snippet.package_path,
            Some(&fragment.absolute_path),
            fragment.note.as_deref(),
            EditorTargetKind::Content,
        );
        let cwd = resolve_editor_cwd(
            self.library.root(),
            &snippet.package_path,
            leaf_dir.as_deref(),
            self.editor_cwd,
        );
        vec![Effect::SpawnEditor {
            request: EditRequest {
                snippet_id: snippet.id,
                target: EditTarget::Content {
                    fragment_id: fragment.id,
                },
                expected: snippet.fingerprint.clone(),
                original: fragment.content.clone(),
                edited: None,
                suffix,
            },
            cwd,
        }]
    }

    pub(super) fn edit_note_effect(&mut self) -> Vec<Effect> {
        // The README has no note; refuse before `mutable_selected` can post a
        // status about a snippet this keypress was never going to touch.
        let Some(index) = self.preview_target.fragment_index() else {
            return Vec::new();
        };
        let Some(snippet) = self.mutable_selected() else {
            return Vec::new();
        };
        let Some(fragment) = snippet.loaded_fragments.get(index) else {
            return Vec::new();
        };
        let leaf_dir = editor_dir_for_target(
            &snippet.package_path,
            Some(&fragment.absolute_path),
            fragment.note.as_deref(),
            EditorTargetKind::Note,
        );
        let cwd = resolve_editor_cwd(
            self.library.root(),
            &snippet.package_path,
            leaf_dir.as_deref(),
            self.editor_cwd,
        );
        vec![Effect::SpawnEditor {
            request: EditRequest {
                snippet_id: snippet.id,
                target: EditTarget::Note {
                    fragment_id: fragment.id,
                },
                expected: snippet.fingerprint.clone(),
                original: fragment.note_content.clone().unwrap_or_default(),
                edited: None,
                suffix: "md".to_owned(),
            },
            cwd,
        }]
    }

    pub(super) fn edit_readme_effect(&mut self) -> Vec<Effect> {
        let Some(snippet) = self.mutable_selected() else {
            return Vec::new();
        };
        let leaf_dir =
            editor_dir_for_target(&snippet.package_path, None, None, EditorTargetKind::Readme);
        let cwd = resolve_editor_cwd(
            self.library.root(),
            &snippet.package_path,
            leaf_dir.as_deref(),
            self.editor_cwd,
        );
        vec![Effect::SpawnEditor {
            request: EditRequest {
                snippet_id: snippet.id,
                target: EditTarget::Readme,
                expected: snippet.fingerprint.clone(),
                original: snippet.readme.clone().unwrap_or_default(),
                edited: None,
                suffix: "md".to_owned(),
            },
            cwd,
        }]
    }

    pub(super) fn copy_content_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .and_then(|snippet| match self.preview_target.fragment_index() {
                Some(index) => {
                    snippet
                        .loaded_fragments
                        .get(index)
                        .map(|fragment| Effect::CopyToClipboard {
                            text: fragment.content.clone(),
                            label: "fragment".to_owned(),
                        })
                }
                None => snippet
                    .readme
                    .as_ref()
                    .map(|readme| Effect::CopyToClipboard {
                        text: readme.clone(),
                        label: "README".to_owned(),
                    }),
            })
            .into_iter()
            .collect()
    }

    pub(super) fn copy_id_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .map(|snippet| Effect::CopyToClipboard {
                text: snippet.id.to_string(),
                label: "snippet ID".to_owned(),
            })
            .into_iter()
            .collect()
    }

    pub(super) fn copy_path_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .map(|snippet| {
                let path = match self.preview_target.fragment_index() {
                    Some(index) => snippet
                        .loaded_fragments
                        .get(index)
                        .map(|fragment| fragment.absolute_path.clone())
                        .unwrap_or_else(|| snippet.package_path.clone()),
                    None => snippet.package_path.join("README.md"),
                };
                Effect::CopyToClipboard {
                    text: path.display().to_string(),
                    label: "snippet path".to_owned(),
                }
            })
            .into_iter()
            .collect()
    }

    pub(super) fn open_gui_editor_effect(&self) -> Vec<Effect> {
        self.selected_snippet()
            .map(|snippet| {
                let path = match self.preview_target.fragment_index() {
                    Some(index) => snippet
                        .loaded_fragments
                        .get(index)
                        .map(|fragment| fragment.absolute_path.clone())
                        .unwrap_or_else(|| snippet.package_path.clone()),
                    None => snippet.package_path.join("README.md"),
                };
                Effect::OpenInGuiEditor { path }
            })
            .into_iter()
            .collect()
    }
}
