use std::sync::mpsc::Sender;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::filesystem::Library;
use crate::gist::gh::Unavailable;

use super::super::super::command::CommandId;
use super::super::super::event::{AppEvent, GistTaskResult};
use super::super::super::modal::{ConfirmModal, InputModal, Modal, ModalAction};
use super::super::super::state::StatusLevel;
use super::super::types::{App, Effect};

pub(crate) enum GistAction {
    Push(crate::gist::PushOptions),
    Attach(String),
    Delete,
    Verify,
}

impl App {
    pub(super) fn handle_gist_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            _ if super::is_ctrl_g(key) => return self.run_command(CommandId::GitOpenConsole),
            _ if super::is_ctrl_s(key) || key.code == KeyCode::Esc => self.gist.open = false,
            _ if super::is_palette_trigger(key) => self.open_palette(),
            KeyCode::Char('p') => return self.run_command(CommandId::GistPush),
            KeyCode::Char('P') => return self.run_command(CommandId::GistPushPublic),
            KeyCode::Char('y') => return self.run_command(CommandId::GistCopyUrl),
            KeyCode::Char('o') => return self.run_command(CommandId::GistOpenInBrowser),
            KeyCode::Char('a') => return self.run_command(CommandId::GistAttach),
            KeyCode::Char('d') => return self.run_command(CommandId::GistDetach),
            KeyCode::Char('x') => return self.run_command(CommandId::GistDelete),
            KeyCode::Char('r') => return self.run_command(CommandId::GistVerifyRemote),
            _ => {}
        }
        Vec::new()
    }

    pub fn set_gist_sender(&mut self, sender: Sender<AppEvent>) {
        self.gist.sender = Some(sender);
    }

    pub(in super::super) fn push_gist(&mut self, public: bool) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(snippet) = self.selected_snippet().cloned() else {
            return Vec::new();
        };
        if public {
            if crate::gist::find(&snippet).is_some() {
                // Visibility is fixed at creation; say so instead of letting the
                // API refuse after a round trip.
                self.set_status(
                    "gist visibility cannot be changed after creation",
                    StatusLevel::Error,
                );
                return Vec::new();
            }
            // A mis-typed `P` would publish the snippet to the whole internet,
            // and GitHub offers no way to walk it back, so this one asks first.
            self.modal = Some(Modal::Confirm(ConfirmModal::new(
                "Publish a public gist?",
                format!(
                    "{:?} becomes readable by anyone and is listed on your GitHub profile. Visibility cannot be changed afterwards.",
                    snippet.title
                ),
                ModalAction::GistPushPublic { id: snippet.id },
                true,
            )));
            return Vec::new();
        }
        self.spawn_push(&snippet, false);
        Vec::new()
    }

    pub(in super::super) fn spawn_push(&mut self, snippet: &crate::domain::Snippet, public: bool) {
        let record = crate::gist::find(snippet).cloned();
        let options = crate::gist::PushOptions {
            public,
            description: record
                .as_ref()
                .and_then(|record| record.description.clone()),
            new: false,
            include_notes: record.as_ref().is_some_and(|record| record.include_notes),
            include_readme: record.as_ref().is_none_or(|record| record.include_readme),
            if_hash: None,
            force: false,
        };
        self.spawn_gist(GistAction::Push(options), snippet.id.to_string());
    }

    pub(in super::super) fn verify_gist(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(snippet) = self.selected_snippet().cloned() else {
            return Vec::new();
        };
        self.spawn_gist(GistAction::Verify, snippet.id.to_string());
        Vec::new()
    }

    pub(in super::super) fn copy_gist_url(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(record) = self.selected_record() else {
            return Vec::new();
        };
        match crate::clipboard::copy(&record.url) {
            Ok(_) => self.set_status("copied gist URL", StatusLevel::Info),
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
        Vec::new()
    }

    pub(in super::super) fn open_gist_in_browser(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(record) = self.selected_record() else {
            return Vec::new();
        };
        match crate::gist::gh::open_web(&record.id) {
            Ok(()) => {}
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
        Vec::new()
    }

    pub(in super::super) fn open_gist_detach_modal(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(snippet) = self.selected_snippet().cloned() else {
            return Vec::new();
        };
        if crate::gist::find(&snippet).is_none() {
            self.set_status("this snippet has no gist", StatusLevel::Error);
            return Vec::new();
        }
        self.modal = Some(Modal::Confirm(ConfirmModal::new(
            "Unlink this gist?",
            "The snippet forgets the link. The gist stays on GitHub, and a later publish creates a new one with a different URL.",
            ModalAction::GistDetach { id: snippet.id },
            false,
        )));
        Vec::new()
    }

    pub(in super::super) fn detach_gist(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(snippet) = self.selected_snippet().cloned() else {
            return Vec::new();
        };
        match crate::gist::detach(&self.library, &snippet.id.to_string()) {
            Ok(_) => match self.rescan() {
                Ok(()) => self.set_status("gist detached", StatusLevel::Info),
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
        Vec::new()
    }

    pub(in super::super) fn open_gist_attach_modal(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(snippet) = self.selected_snippet().cloned() else {
            return Vec::new();
        };
        self.modal = Some(Modal::Input(InputModal::new(
            "Gist ID or URL",
            "",
            ModalAction::GistAttach { id: snippet.id },
        )));
        Vec::new()
    }

    pub(in super::super) fn open_gist_delete_modal(&mut self) -> Vec<Effect> {
        if self.refuse_busy_gist() {
            return Vec::new();
        }
        let Some(snippet) = self.selected_snippet().cloned() else {
            return Vec::new();
        };
        self.modal = Some(Modal::Confirm(ConfirmModal::new(
            "Delete gist on GitHub?",
            "The gist is deleted for everyone you shared the link with. The snippet stays in your library.",
            ModalAction::GistDelete { id: snippet.id },
            true,
        )));
        Vec::new()
    }

    pub(in super::super) fn toggle_published_filter(&mut self) {
        self.filter.published = !self.filter.published;
        self.refresh_visible();
        self.set_status(
            if self.filter.published {
                "showing published snippets only"
            } else {
                "showing all snippets"
            },
            StatusLevel::Info,
        );
    }

    fn refuse_busy_gist(&mut self) -> bool {
        if self.gist.in_flight {
            self.set_status("a background gist operation is running", StatusLevel::Error);
            true
        } else {
            false
        }
    }

    fn selected_record(&self) -> Option<crate::domain::RemoteRecord> {
        self.selected_snippet().and_then(crate::gist::find).cloned()
    }

    pub(crate) fn spawn_gist(&mut self, action: GistAction, id: String) {
        let Some(sender) = self.gist.sender.clone() else {
            self.set_status("gist is unavailable", StatusLevel::Error);
            return;
        };
        self.gist.in_flight = true;
        let (action_name, pending) = match &action {
            GistAction::Push(_) => ("push", "publishing…"),
            GistAction::Attach(_) => ("attach", "attaching…"),
            GistAction::Delete => ("delete", "deleting…"),
            GistAction::Verify => ("verify", "verifying…"),
        };
        self.set_status(pending, StatusLevel::Info);
        let library = self.library.clone();
        std::thread::spawn(move || {
            let outcome = run_gist_action(&library, &id, action);
            let _ = sender.send(AppEvent::GistFinished(GistTaskResult {
                action: action_name,
                outcome,
            }));
        });
    }

    pub fn handle_gist_task(&mut self, result: GistTaskResult) {
        self.gist.in_flight = false;
        match result.outcome {
            Ok(message) => {
                self.gist.unavailable = None;
                self.gist.last_error = None;
                match self.rescan() {
                    Ok(()) => self.set_status(message, StatusLevel::Info),
                    Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
                }
            }
            Err(message) => {
                match message.as_str() {
                    "gh not found in PATH" => {
                        self.gist.unavailable = Some(Unavailable::BinaryMissing)
                    }
                    "gh is not authenticated" => {
                        self.gist.unavailable = Some(Unavailable::NotAuthenticated)
                    }
                    _ => {}
                }
                self.gist.last_error = Some(message.clone());
                self.set_status(message, StatusLevel::Error);
            }
        }
        if self.pending_quit {
            self.should_quit = true;
        }
    }
}

fn run_gist_action(
    library: &Library,
    id: &str,
    action: GistAction,
) -> std::result::Result<String, String> {
    match action {
        GistAction::Push(options) => {
            crate::gist::push(library, id, &options).map(|outcome| match outcome.action() {
                "created" => "gist created".to_owned(),
                "updated" => "gist updated".to_owned(),
                _ => "gist is already up to date".to_owned(),
            })
        }
        GistAction::Attach(gist) => {
            crate::gist::attach(library, id, &gist).map(|_| "gist attached".to_owned())
        }
        GistAction::Delete => crate::gist::delete(library, id).map(|_| "gist deleted".to_owned()),
        GistAction::Verify => verify_remote(library, id),
    }
    .map_err(|error| error.to_string())
}

fn verify_remote(library: &Library, id: &str) -> Result<String, crate::error::SnipError> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, id)?;
    let Some(record) = crate::gist::find(snippet) else {
        return Err(crate::error::SnipError::usage("this snippet has no gist"));
    };
    match crate::gist::gh::fetch(&record.id) {
        Ok(_) => Ok("gist is still on GitHub".to_owned()),
        Err(error) if error.kind == crate::error::ErrorKind::NotFound => Err(
            crate::error::SnipError::not_found("gist no longer exists on GitHub"),
        ),
        Err(error) => Err(error),
    }
}
