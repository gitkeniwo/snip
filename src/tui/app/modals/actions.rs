use crate::error::{Result, SnipError};
use crate::git::GitAction;
use crate::service::{
    CreateOptions, EditOptions, FragmentEditOptions, create_folder, create_snippet, delete_folder,
    delete_snippet, delete_tag, edit_fragment, edit_snippet, move_folder, purge_snippet,
    remove_fragment, rename_tag,
};

use crate::tui::app::input::gist::GistAction;
use crate::tui::app::types::{App, Effect};
use crate::tui::editor::{EditRequest, EditTarget};
use crate::tui::modal::{Modal, ModalAction, PickerModal};
use crate::tui::state::{Pane, StatusLevel};

impl App {
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
            ModalAction::RenameFragment { id, fragment_index } => {
                let title = input()?;
                if title.is_empty() {
                    return Err(SnipError::usage("fragment title cannot be empty"));
                }
                edit_fragment(
                    &self.library,
                    &id.to_string(),
                    &(fragment_index + 1).to_string(),
                    &FragmentEditOptions {
                        title: Some(title.to_owned()),
                        ..FragmentEditOptions::default()
                    },
                )?;
                "fragment renamed".to_owned()
            }
            ModalAction::DeleteFragment { id, fragment_index } => {
                remove_fragment(
                    &self.library,
                    &id.to_string(),
                    &(fragment_index + 1).to_string(),
                    None,
                    false,
                )?;
                "fragment deleted".to_owned()
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
            ModalAction::EditLanguage { id, fragment_index } => {
                edit_snippet(
                    &self.library,
                    &id.to_string(),
                    &EditOptions {
                        fragment_selector: Some((fragment_index + 1).to_string()),
                        language: Some(input()?.to_owned()),
                        ..EditOptions::default()
                    },
                )?;
                "language updated".to_owned()
            }
            ModalAction::DeleteSnippet { id } => {
                delete_snippet(&self.library, &id.to_string(), None, false)?;
                "snippet moved to trash".to_owned()
            }
            ModalAction::ForceEdit(request) => {
                return Ok((vec![Effect::ForceSave(request)], String::new()));
            }
            ModalAction::GitCommit => {
                let message = input()?;
                if message.is_empty() {
                    return Err(SnipError::usage("commit message cannot be empty"));
                }
                self.git_effect(GitAction::Commit {
                    message: Some(message.to_owned()),
                });
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::GitAutoCommitInterval => {
                let minutes = input()?.parse::<u32>().map_err(|_| {
                    SnipError::usage("automatic commit interval must be whole minutes")
                })?;
                self.persist_git_settings(Some(minutes), None, None)?;
                return Ok((Vec::new(), format!("automatic interval: {minutes} min")));
            }
            ModalAction::PickTheme => {
                let name = input()?;
                self.preview_theme(name)?;
                self.theme_preview = None;
                let appearance = self.theme_source.appearance;
                let mut config = match crate::config::AppConfig::load_from(&self.config_path) {
                    Ok(config) => config,
                    Err(error) => {
                        self.set_status(
                            format!("theme changed for this session: {error}"),
                            StatusLevel::Error,
                        );
                        return Ok((Vec::new(), String::new()));
                    }
                };
                let tui = config
                    .tui
                    .get_or_insert_with(crate::config::TuiConfig::default);
                match appearance {
                    crate::theme::Appearance::Light => {
                        tui.light_theme = Some(name.to_owned());
                        self.theme_config.light_theme = Some(name.to_owned());
                    }
                    crate::theme::Appearance::Dark => {
                        tui.dark_theme = Some(name.to_owned());
                        self.theme_config.dark_theme = Some(name.to_owned());
                    }
                }
                if let Err(error) = config.save_to(&self.config_path) {
                    self.set_status(
                        format!("theme changed for this session: {error}"),
                        StatusLevel::Error,
                    );
                    return Ok((Vec::new(), String::new()));
                }
                return Ok((Vec::new(), format!("theme: {name}")));
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
                    .items()
                    .iter()
                    .position(|item| item.value == preferred)
                    .unwrap_or(0);
                self.modal = Some(Modal::Picker(picker));
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::CreateFolder { title } => {
                let current = self.default_language.clone();
                let mut picker = PickerModal::new(
                    "Language",
                    self.language_picker_items(),
                    ModalAction::CreateLanguage {
                        title,
                        folder: input()?.to_owned(),
                    },
                )
                .allow_custom()
                .with_current_value(current.clone());
                if let Some(language) = crate::language::info(&current) {
                    picker.select_value(language.aliases[0]);
                } else {
                    picker.set_filter(current);
                }
                self.modal = Some(Modal::Picker(picker));
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
            ModalAction::GistAttach { id } => {
                let gist = input()?.to_owned();
                crate::gist::parse_gist_id(&gist)?;
                self.spawn_gist(GistAction::Attach(gist), id.to_string());
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::GistDelete { id } => {
                self.spawn_gist(GistAction::Delete, id.to_string());
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::GistDetach { .. } => {
                self.detach_gist();
                return Ok((Vec::new(), String::new()));
            }
            ModalAction::GistPushPublic { id } => {
                let snippet = self
                    .catalog
                    .snippets
                    .iter()
                    .find(|snippet| snippet.id == id)
                    .cloned();
                if let Some(snippet) = snippet {
                    self.spawn_push(&snippet, true);
                }
                return Ok((Vec::new(), String::new()));
            }
        };
        self.rescan()?;
        Ok((Vec::new(), message))
    }
}
