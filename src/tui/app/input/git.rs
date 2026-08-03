use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::super::super::command::CommandId;
use crate::git::{self, GitAction, Refusal, Unavailable};

use super::super::super::modal::{InputModal, Modal, ModalAction};
use super::super::super::state::StatusLevel;
use super::super::types::{App, Effect};

impl App {
    pub(super) fn handle_git_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            _ if super::is_ctrl_s(key) => return self.run_command(CommandId::GistOpenPanel),
            _ if super::is_ctrl_g(key) || key.code == KeyCode::Esc => self.git.open = false,
            _ if super::is_palette_trigger(key) => self.open_palette(),
            KeyCode::Char('r') => return self.run_command(CommandId::GitRefreshLocalStatus),
            KeyCode::Char('b') => return self.run_command(CommandId::GitBackup),
            KeyCode::Char('c') => return self.run_command(CommandId::GitCommit),
            KeyCode::Char('p') => return self.run_command(CommandId::GitPush),
            KeyCode::Char('f') => return self.run_command(CommandId::GitFetchRemoteStatus),
            KeyCode::Char('l') => return self.run_command(CommandId::GitPull),
            KeyCode::Char('C') => return self.run_command(CommandId::GitCommitWithMessage),
            KeyCode::Char('a') => return self.run_command(CommandId::GitPauseAutoBackup),
            KeyCode::Char('u') => return self.run_command(CommandId::GitToggleAutoPush),
            KeyCode::Char('U') => return self.run_command(CommandId::GitToggleAutoPull),
            KeyCode::Char('o') => return self.run_command(CommandId::GitToggleBackupOnQuit),
            KeyCode::Char('i') => {
                if matches!(
                    self.git.unavailable.as_ref(),
                    Some(Unavailable::NotARepository)
                ) {
                    return self.run_command(CommandId::GitInitRepository);
                }
                return self.run_command(CommandId::GitSetAutoCommitInterval);
            }
            _ => {}
        }
        Vec::new()
    }

    pub(in super::super) fn git_effect(&mut self, action: GitAction) -> Vec<Effect> {
        if self.git.push_in_flight || self.git.pull_in_flight || self.git.fetch_in_flight {
            self.set_status(
                if self.git.push_in_flight {
                    "a background push is running"
                } else if self.git.pull_in_flight {
                    "a background pull is running"
                } else {
                    "a background fetch is running"
                },
                StatusLevel::Error,
            );
            return Vec::new();
        }
        let refusal =
            match &action {
                GitAction::Init => match self.git.unavailable.as_ref() {
                    Some(Unavailable::NotARepository) => None,
                    Some(Unavailable::ProbeFailed { .. }) => Some(Refusal::ProbeFailed),
                    Some(Unavailable::BinaryMissing) => Some(Refusal::Unavailable),
                    None => {
                        self.set_status(
                            "this library is already a Git repository",
                            StatusLevel::Error,
                        );
                        return Vec::new();
                    }
                },
                GitAction::Backup => self.git.status.as_ref().map_or(
                    Some(self.git_availability_refusal()),
                    |status| match git::check_backup(status) {
                        Err(Refusal::NothingToCommit) | Ok(()) => None,
                        Err(refusal) => Some(refusal),
                    },
                ),
                GitAction::Commit { .. } => self
                    .git
                    .status
                    .as_ref()
                    .map_or(Some(self.git_availability_refusal()), |status| {
                        git::check_commit(status).err()
                    }),
                GitAction::Push => self
                    .git
                    .status
                    .as_ref()
                    .map_or(Some(self.git_availability_refusal()), |status| {
                        git::check_push(status).err()
                    }),
                GitAction::Pull => self
                    .git
                    .status
                    .as_ref()
                    .map_or(Some(self.git_availability_refusal()), |status| {
                        git::check_pull(status, status.dirty_count()).err()
                    }),
            };
        if let Some(refusal) = refusal {
            self.set_status(refusal.to_string(), StatusLevel::Error);
            return Vec::new();
        }
        self.git.operation_queued = true;
        if matches!(action, GitAction::Init) {
            vec![Effect::RunGit(action)]
        } else {
            self.spawn_git_operation(action);
            Vec::new()
        }
    }

    pub(in super::super) fn request_quit(&mut self) -> Vec<Effect> {
        if self.git.push_in_flight
            || self.git.pull_in_flight
            || self.git.fetch_in_flight
            || self.gist.in_flight
        {
            self.pending_quit = true;
            self.set_status(
                if self.gist.in_flight {
                    "finishing gist operation…"
                } else if self.git.push_in_flight {
                    "finishing background push…"
                } else if self.git.pull_in_flight {
                    "finishing background pull…"
                } else {
                    "finishing background fetch…"
                },
                StatusLevel::Info,
            );
            return Vec::new();
        }
        if self.git.backup_on_quit {
            // The periodic badge may be a few seconds old; quitting is the one
            // point where the backup decision must use a fresh worktree view.
            self.refresh_git();
        }
        let should_backup = self.git.backup_on_quit
            && self
                .git
                .status
                .as_ref()
                .is_some_and(|status| git::check_backup(status).is_ok());
        if should_backup {
            self.pending_quit = true;
            self.git.operation_queued = true;
            self.spawn_git_operation(GitAction::Backup);
            Vec::new()
        } else {
            self.should_quit = true;
            Vec::new()
        }
    }

    pub(crate) fn resume_quit_after_push(&mut self) -> Vec<Effect> {
        self.pending_quit = false;
        self.request_quit()
    }

    pub(in super::super) fn toggle_auto_backup(&mut self) {
        if self.git.auto_commit_interval == 0 {
            self.set_status(
                "automatic Git operations are off; set git-auto-commit-interval to enable them",
                StatusLevel::Info,
            );
            return;
        }
        self.git.auto_backup_paused = !self.git.auto_backup_paused;
        self.set_status(
            if self.git.auto_backup_paused {
                "automatic Git operations paused for this session"
            } else {
                "automatic Git operations resumed"
            },
            StatusLevel::Info,
        );
    }

    pub(in super::super) fn open_git_message(&mut self) {
        if self.git.push_in_flight || self.git.pull_in_flight || self.git.fetch_in_flight {
            self.set_status(
                if self.git.push_in_flight {
                    "a background push is running"
                } else if self.git.pull_in_flight {
                    "a background pull is running"
                } else {
                    "a background fetch is running"
                },
                StatusLevel::Error,
            );
            return;
        }
        let Some(status) = self.git.status.as_ref() else {
            self.set_status(
                self.git_availability_refusal().to_string(),
                StatusLevel::Error,
            );
            return;
        };
        if let Err(refusal) = git::check_commit(status) {
            self.set_status(refusal.to_string(), StatusLevel::Error);
            return;
        }
        self.modal = Some(Modal::Input(InputModal::new(
            "Commit message",
            git::backup_message(status),
            ModalAction::GitCommit,
        )));
    }

    pub(in super::super) fn open_auto_commit_interval(&mut self) {
        self.modal = Some(Modal::Input(InputModal::new(
            "Automatic commit interval (minutes; 0 disables)",
            self.git.auto_commit_interval.to_string(),
            ModalAction::GitAutoCommitInterval,
        )));
    }

    pub(in super::super) fn toggle_auto_push(&mut self) {
        let next = !self.git.auto_push;
        match self.persist_git_settings(None, Some(next), None, None) {
            Ok(()) => self.set_status(
                if next {
                    "automatic push enabled"
                } else {
                    "automatic push disabled"
                },
                StatusLevel::Info,
            ),
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(in super::super) fn toggle_auto_pull(&mut self) {
        let next = !self.git.auto_pull;
        match self.persist_git_settings(None, None, Some(next), None) {
            Ok(()) => self.set_status(
                if next {
                    "automatic pull on start enabled"
                } else {
                    "automatic pull on start disabled"
                },
                StatusLevel::Info,
            ),
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(in super::super) fn toggle_backup_on_quit(&mut self) {
        let next = !self.git.backup_on_quit;
        match self.persist_git_settings(None, None, None, Some(next)) {
            Ok(()) => self.set_status(
                if next {
                    "backup on quit enabled"
                } else {
                    "backup on quit disabled"
                },
                StatusLevel::Info,
            ),
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(in super::super) fn persist_git_settings(
        &mut self,
        interval: Option<u32>,
        auto_push: Option<bool>,
        auto_pull: Option<bool>,
        backup_on_quit: Option<bool>,
    ) -> crate::error::Result<()> {
        let mut config = crate::config::AppConfig::load()?;
        let git = config
            .git
            .get_or_insert_with(crate::config::GitConfig::default);
        if let Some(interval) = interval {
            git.auto_commit_interval = interval;
        }
        if let Some(auto_push) = auto_push {
            git.auto_push = auto_push;
        }
        if let Some(auto_pull) = auto_pull {
            git.auto_pull = auto_pull;
        }
        if let Some(backup_on_quit) = backup_on_quit {
            git.backup_on_quit = backup_on_quit;
        }
        config.save()?;
        if let Some(interval) = interval {
            self.git.auto_commit_interval = interval;
        }
        if let Some(auto_push) = auto_push {
            self.git.auto_push = auto_push;
        }
        if let Some(auto_pull) = auto_pull {
            self.git.auto_pull = auto_pull;
        }
        if let Some(backup_on_quit) = backup_on_quit {
            self.git.backup_on_quit = backup_on_quit;
        }
        Ok(())
    }

    fn git_availability_refusal(&self) -> Refusal {
        match self.git.unavailable.as_ref() {
            Some(Unavailable::ProbeFailed { .. }) => Refusal::ProbeFailed,
            Some(Unavailable::BinaryMissing | Unavailable::NotARepository) | None => {
                Refusal::Unavailable
            }
        }
    }
}
