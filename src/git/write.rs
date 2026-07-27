use std::path::Path;

use serde::Serialize;

use crate::error::{Result, SnipError};
use crate::filesystem::Library;

use super::{
    GitAction, Repo, check_commit, check_push, command, command_failed, probe, refusal_error,
    spawn_error, status,
};

#[derive(Clone, Copy)]
enum Mode {
    Interactive,
    NonInteractive,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActionOutcome {
    pub action: &'static str,
    pub committed: bool,
    pub pushed: bool,
    pub message: String,
}

pub fn init(library_root: &Path) -> Result<()> {
    init_with(library_root, Mode::NonInteractive)
}

pub fn commit(repo: &Repo, message: &str) -> Result<()> {
    check_commit(&status(repo)?).map_err(refusal_error)?;
    commit_with(repo, message, Mode::NonInteractive)
}

pub fn push(repo: &Repo) -> Result<()> {
    check_push(&status(repo)?).map_err(refusal_error)?;
    push_with(repo, Mode::NonInteractive)
}

pub fn backup(repo: &Repo, message: &str) -> Result<ActionOutcome> {
    let before = status(repo)?;
    backup_with(repo, message, Mode::NonInteractive, &before)
}

pub fn execute_interactive(library_root: &Path, action: &GitAction) -> Result<ActionOutcome> {
    if matches!(action, GitAction::Init) {
        init_with(library_root, Mode::Interactive)?;
        return Ok(ActionOutcome {
            action: "init",
            committed: false,
            pushed: false,
            message: "git repository initialized".to_owned(),
        });
    }
    let repo = probe(library_root).map_err(super::unavailable_error)?;
    let current = status(&repo)?;
    match action {
        GitAction::Backup => {
            check_commit(&current).map_err(refusal_error)?;
            let message = super::backup_message(&current);
            backup_with(&repo, &message, Mode::Interactive, &current)
        }
        GitAction::Commit { message } => {
            check_commit(&current).map_err(refusal_error)?;
            let message = message
                .clone()
                .unwrap_or_else(|| super::backup_message(&current));
            commit_with(&repo, &message, Mode::Interactive)?;
            Ok(ActionOutcome {
                action: "commit",
                committed: true,
                pushed: false,
                message: "backup committed".to_owned(),
            })
        }
        GitAction::Push => {
            check_push(&current).map_err(refusal_error)?;
            push_with(&repo, Mode::Interactive)?;
            Ok(ActionOutcome {
                action: "push",
                committed: false,
                pushed: true,
                message: "backup pushed".to_owned(),
            })
        }
        GitAction::Init => unreachable!(),
    }
}

fn init_with(library_root: &Path, mode: Mode) -> Result<()> {
    run(mode, library_root, &["init"], "git init")
}

fn commit_with(repo: &Repo, message: &str, mode: Mode) -> Result<()> {
    let library = Library::open(&repo.library_root)?;
    // This lock is defensive rather than format-mandated: it prevents git add
    // from observing a package halfway through another snip process's write.
    // It deliberately ends before any network operation.
    let library_lock = library.lock()?;
    run(
        mode,
        &repo.library_root,
        &["add", "-A", "--", "."],
        "git add",
    )?;
    let result = if repo.library_prefix.is_some() {
        run(
            mode,
            &repo.library_root,
            &["commit", "-m", message, "--", "."],
            "git commit",
        )
    } else {
        run(
            mode,
            &repo.library_root,
            &["commit", "-m", message],
            "git commit",
        )
    };
    drop(library_lock);
    result
}

fn push_with(repo: &Repo, mode: Mode) -> Result<()> {
    run(mode, &repo.library_root, &["push"], "git push").map_err(push_error)
}

fn backup_with(
    repo: &Repo,
    message: &str,
    mode: Mode,
    before: &super::Status,
) -> Result<ActionOutcome> {
    check_commit(before).map_err(refusal_error)?;
    commit_with(repo, message, mode)?;
    let after = status(repo)?;
    let pushed = after.upstream.is_some() && after.ahead > 0;
    if pushed {
        push_with(repo, mode).map_err(|error| {
            SnipError::io(format!("backup was committed, but push failed: {error}"))
        })?;
    }
    Ok(ActionOutcome {
        action: "backup",
        committed: true,
        pushed,
        message: if pushed {
            "backup committed and pushed".to_owned()
        } else if after.upstream.is_none() {
            "backup committed; set an upstream to enable push".to_owned()
        } else {
            "backup committed".to_owned()
        },
    })
}

fn push_error(error: SnipError) -> SnipError {
    // Interactive Git inherits stderr so people see Git's own localized hint;
    // only non-interactive failures carry text that can match these substrings.
    let lower = error.message.to_ascii_lowercase();
    if lower.contains("non-fast-forward") || lower.contains("fetch first") {
        SnipError::io(format!(
            "{error}; pull in your terminal, then try the push again"
        ))
    } else {
        error
    }
}

#[cfg(feature = "tui")]
pub(crate) fn is_library_lock_conflict(error: &SnipError) -> bool {
    error.kind == crate::error::ErrorKind::Conflict
        && error
            .message
            .starts_with("library is locked by another process:")
}

fn run(mode: Mode, cwd: &Path, args: &[&str], label: &str) -> Result<()> {
    match mode {
        Mode::Interactive => {
            let status = command::run_interactive(cwd, args).map_err(spawn_error)?;
            if status.success() {
                Ok(())
            } else {
                Err(command_failed(
                    label,
                    &format!("exited with status {status}"),
                ))
            }
        }
        Mode::NonInteractive => {
            let output = command::run_non_interactive(cwd, args).map_err(spawn_error)?;
            if output.status.success() {
                Ok(())
            } else {
                Err(command_failed(label, &output.stderr))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_hint_is_limited_to_remote_ahead_rejections() {
        let rejected = push_error(SnipError::io(
            "git push failed: ! [rejected] main -> main (fetch first)",
        ));
        assert!(rejected.message.contains("pull in your terminal"));

        let authentication = push_error(SnipError::io(
            "git push failed: authentication failed for remote",
        ));
        assert_eq!(
            authentication.message,
            "git push failed: authentication failed for remote"
        );

        let missing_git = push_error(SnipError::io("git not found in PATH"));
        assert_eq!(missing_git.message, "git not found in PATH");
    }
}
