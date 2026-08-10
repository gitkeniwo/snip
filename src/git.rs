mod command;
mod parse;
mod write;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use time::{OffsetDateTime, UtcOffset};

use crate::error::{Result, SnipError};

pub use command::{Output as CommandOutput, SpawnError, run_non_interactive};
pub use parse::relative_time;
pub use write::{
    ActionOutcome, PullOutcome, backup, commit, execute_interactive, fetch, init, pull,
    pull_message, push,
};
#[cfg(feature = "tui")]
pub(crate) use write::{execute_non_interactive, is_library_lock_conflict, is_pull_refusal};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Unavailable {
    BinaryMissing,
    NotARepository,
    ProbeFailed { message: String },
}

#[derive(Clone, Debug)]
pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub library_prefix: Option<String>,
    pub(crate) library_root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    Unavailable,
    ProbeFailed,
    Conflicted,
    MidOperation(RepoState),
    DetachedHead,
    NothingToCommit,
    NoUpstream,
    NothingToPush,
    DirtyWorktree,
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "git is unavailable",
            Self::ProbeFailed => "git could not inspect this repository",
            Self::Conflicted => "resolve Git conflicts in your terminal before continuing",
            Self::MidOperation(_) => "finish the current Git operation in your terminal first",
            Self::DetachedHead => "Git HEAD is detached; switch to a branch before continuing",
            Self::NothingToCommit => "there are no library changes to commit",
            Self::NoUpstream => {
                "no upstream is configured; run git push -u origin <branch> once in your terminal"
            }
            Self::NothingToPush => "there are no commits to push",
            Self::DirtyWorktree => "commit or stash your changes before pulling",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GitAction {
    Backup,
    Commit { message: Option<String> },
    Push,
    Pull,
    Init,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Branch {
    Named { name: String },
    Detached { short_id: String },
    Unborn,
}

impl Branch {
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named { name } => Some(name),
            Self::Detached { .. } | Self::Unborn => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoState {
    #[default]
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}

impl RepoState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Merging => "merge",
            Self::Rebasing => "rebase",
            Self::CherryPicking => "cherry-pick",
            Self::Reverting => "revert",
            Self::Bisecting => "bisect",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Commit {
    pub short_id: String,
    pub timestamp: i64,
    pub subject: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Status {
    pub branch: Branch,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicted: Vec<String>,
    pub state: RepoState,
    pub head_oid: Option<String>,
    pub last_commit: Option<Commit>,
    pub upstream_commit: Option<Commit>,
}

impl Status {
    pub fn dirty_count(&self) -> usize {
        self.staged + self.unstaged + self.untracked
    }
}

pub fn check_commit(status: &Status) -> std::result::Result<(), Refusal> {
    if !status.conflicted.is_empty() {
        return Err(Refusal::Conflicted);
    }
    if status.state != RepoState::Clean {
        return Err(Refusal::MidOperation(status.state));
    }
    if matches!(status.branch, Branch::Detached { .. }) {
        return Err(Refusal::DetachedHead);
    }
    if status.dirty_count() == 0 {
        return Err(Refusal::NothingToCommit);
    }
    Ok(())
}

pub fn check_push(status: &Status) -> std::result::Result<(), Refusal> {
    if !status.conflicted.is_empty() {
        return Err(Refusal::Conflicted);
    }
    if status.state != RepoState::Clean {
        return Err(Refusal::MidOperation(status.state));
    }
    if matches!(status.branch, Branch::Detached { .. }) {
        return Err(Refusal::DetachedHead);
    }
    if status.upstream.is_none() {
        return Err(Refusal::NoUpstream);
    }
    if status.ahead == 0 {
        return Err(Refusal::NothingToPush);
    }
    Ok(())
}

pub fn check_pull(status: &Status, repo_dirty: usize) -> std::result::Result<(), Refusal> {
    if !status.conflicted.is_empty() {
        return Err(Refusal::Conflicted);
    }
    if status.state != RepoState::Clean {
        return Err(Refusal::MidOperation(status.state));
    }
    if matches!(status.branch, Branch::Detached { .. }) {
        return Err(Refusal::DetachedHead);
    }
    if matches!(status.branch, Branch::Unborn) {
        return Err(Refusal::NothingToCommit);
    }
    if status.upstream.is_none() {
        return Err(Refusal::NoUpstream);
    }
    if repo_dirty > 0 {
        return Err(Refusal::DirtyWorktree);
    }
    Ok(())
}

/// Checks whether a backup has work to perform.
///
/// `NothingToCommit` means no local commit or push can be performed from the
/// current status. `backup` treats that state as a successful no-op (while
/// still reporting a missing upstream); callers such as backup-on-quit use
/// this predicate to avoid launching an unnecessary action.
pub fn check_backup(status: &Status) -> std::result::Result<(), Refusal> {
    match check_commit(status) {
        Ok(()) => Ok(()),
        Err(Refusal::NothingToCommit) => match check_push(status) {
            Ok(()) => Ok(()),
            Err(Refusal::NoUpstream | Refusal::NothingToPush) => Err(Refusal::NothingToCommit),
            Err(refusal) => Err(refusal),
        },
        Err(refusal) => Err(refusal),
    }
}

pub fn should_auto_backup(status: &Status, now: i64, interval_minutes: u32, paused: bool) -> bool {
    if paused || interval_minutes == 0 || check_commit(status).is_err() {
        return false;
    }
    let interval_seconds = i64::from(interval_minutes) * 60;
    status
        .last_commit
        .as_ref()
        .is_none_or(|commit| now.saturating_sub(commit.timestamp) >= interval_seconds)
}

pub fn should_auto_push(status: &Status, enabled: bool, paused: bool) -> bool {
    enabled && !paused && check_push(status).is_ok()
}

pub fn backup_message(status: &Status) -> String {
    let now = OffsetDateTime::now_utc();
    let local = UtcOffset::current_local_offset()
        .map(|offset| now.to_offset(offset))
        .unwrap_or(now);
    backup_message_at(status, local)
}

pub fn backup_message_at(status: &Status, timestamp: OffsetDateTime) -> String {
    let count = status.dirty_count();
    let unit = if count == 1 { "file" } else { "files" };
    format!(
        "snip backup: {:04}-{:02}-{:02} {:02}:{:02} ({count} {unit})",
        timestamp.year(),
        u8::from(timestamp.month()),
        timestamp.day(),
        timestamp.hour(),
        timestamp.minute()
    )
}

pub fn probe(library_root: &Path) -> std::result::Result<Repo, Unavailable> {
    let output = command::run(
        library_root,
        &["rev-parse", "--show-toplevel", "--absolute-git-dir"],
    )
    .map_err(|error| match error {
        command::SpawnError::NotInstalled => Unavailable::BinaryMissing,
        command::SpawnError::Io(message) => Unavailable::ProbeFailed {
            message: format!("cannot run git: {message}"),
        },
    })?;
    if !output.status.success() {
        return Err(if output.stderr.contains("not a git repository") {
            Unavailable::NotARepository
        } else {
            Unavailable::ProbeFailed {
                message: if output.stderr.is_empty() {
                    format!("git rev-parse failed with status {}", output.status)
                } else {
                    format!("git rev-parse failed: {}", output.stderr)
                },
            }
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let root = lines
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| Unavailable::ProbeFailed {
            message: "git rev-parse did not return a repository root".to_owned(),
        })?;
    let git_dir = lines
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| Unavailable::ProbeFailed {
            message: "git rev-parse did not return an absolute Git directory".to_owned(),
        })?;
    let library_root = canonicalize_probe_path(library_root, "library root")?;
    let root = canonicalize_probe_path(&root, "repository root")?;
    let git_dir = canonicalize_probe_path(&git_dir, "Git directory")?;
    let library_prefix = library_root
        .strip_prefix(&root)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    Ok(Repo {
        root,
        git_dir,
        library_prefix,
        library_root,
    })
}

pub fn status(repo: &Repo) -> Result<Status> {
    let output = command::run(
        &repo.library_root,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            // `normal` intentionally counts a new snippet package as one entry
            // instead of inflating the badge with every file inside it.
            "--untracked-files=normal",
            "--",
            ".",
        ],
    )
    .map_err(spawn_error)?;
    // The pathspec scopes file counts to the library; ahead/behind remains
    // repository-wide because branch relationships do not have a path scope.
    if !output.status.success() {
        return Err(command_failed("git status", &output.stderr));
    }
    let parsed = parse::parse_status_v2(&output.stdout);
    let branch = parsed.branch.unwrap_or_else(|| Branch::Detached {
        short_id: parsed
            .head_oid
            .as_deref()
            .unwrap_or("unknown")
            .chars()
            .take(7)
            .collect(),
    });
    let (last_commit, upstream_commit) = if parsed.head_oid.is_some() {
        load_commits(
            repo,
            parsed.upstream.is_some(),
            parsed.ahead == 0 && parsed.behind == 0,
        )?
    } else {
        (None, None)
    };
    let conflicted = parsed
        .conflicted
        .into_iter()
        .map(|path| strip_library_prefix(repo, &path))
        .collect();
    Ok(Status {
        branch,
        upstream: parsed.upstream,
        ahead: parsed.ahead,
        behind: parsed.behind,
        staged: parsed.staged,
        unstaged: parsed.unstaged,
        untracked: parsed.untracked,
        conflicted,
        state: repo_state(&repo.git_dir),
        head_oid: parsed.head_oid,
        last_commit,
        upstream_commit,
    })
}

fn repo_dirty_count(repo: &Repo) -> Result<usize> {
    let output = command::run(
        &repo.root,
        &["status", "--porcelain=v2", "-z", "--untracked-files=normal"],
    )
    .map_err(spawn_error)?;
    if !output.status.success() {
        return Err(command_failed("git status", &output.stderr));
    }
    let parsed = parse::parse_status_v2(&output.stdout);
    Ok(parsed.staged + parsed.unstaged + parsed.untracked)
}

fn canonicalize_probe_path(path: &Path, label: &str) -> std::result::Result<PathBuf, Unavailable> {
    fs::canonicalize(path).map_err(|error| Unavailable::ProbeFailed {
        message: format!("cannot resolve {label} {}: {error}", path.display()),
    })
}

fn load_commits(
    repo: &Repo,
    include_upstream: bool,
    synchronized: bool,
) -> Result<(Option<Commit>, Option<Commit>)> {
    let mut arguments = vec!["show", "-s", "--format=%x1e%h%x00%ct%x00%s", "HEAD"];
    if include_upstream {
        arguments.push("@{upstream}");
    }
    let output = command::run(&repo.root, &arguments).map_err(spawn_error)?;
    if !output.status.success() {
        // A configured upstream may outlive its pruned remote-tracking ref.
        // Preserve valid local metadata instead of letting that missing second
        // revision make the combined query discard HEAD as well.
        return if include_upstream {
            Ok((load_commit(repo, "HEAD")?, None))
        } else {
            Ok((None, None))
        };
    }
    let mut commits = parse::parse_logs(&output.stdout).into_iter();
    let local = commits.next();
    // `git show HEAD @{upstream}` deduplicates identical objects. When status
    // says the refs are synchronized, that one record represents both sides.
    let upstream = include_upstream
        .then(|| {
            commits
                .next()
                .or_else(|| synchronized.then(|| local.clone()).flatten())
        })
        .flatten();
    Ok((local, upstream))
}

fn load_commit(repo: &Repo, reference: &str) -> Result<Option<Commit>> {
    let output = command::run(
        &repo.root,
        &["show", "-s", "--format=%h%x00%ct%x00%s", reference],
    )
    .map_err(spawn_error)?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(parse::parse_log(&output.stdout))
}

fn repo_state(git_dir: &Path) -> RepoState {
    if git_dir.join("MERGE_HEAD").exists() {
        RepoState::Merging
    } else if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        RepoState::Rebasing
    } else if git_dir.join("CHERRY_PICK_HEAD").exists() {
        RepoState::CherryPicking
    } else if git_dir.join("REVERT_HEAD").exists() {
        RepoState::Reverting
    } else if git_dir.join("BISECT_LOG").exists() {
        RepoState::Bisecting
    } else {
        RepoState::Clean
    }
}

fn strip_library_prefix(repo: &Repo, path: &str) -> String {
    repo.library_prefix
        .as_deref()
        .and_then(|prefix| path.strip_prefix(prefix))
        .and_then(|path| path.strip_prefix('/'))
        .unwrap_or(path)
        .to_owned()
}

fn spawn_error(error: command::SpawnError) -> SnipError {
    match error {
        command::SpawnError::NotInstalled => SnipError::io("git not found in PATH"),
        command::SpawnError::Io(message) => SnipError::io(format!("cannot run git: {message}")),
    }
}

fn command_failed(label: &str, stderr: &str) -> SnipError {
    if stderr.is_empty() {
        SnipError::io(format!("{label} failed"))
    } else {
        SnipError::io(format!("{label} failed: {stderr}"))
    }
}

fn refusal_error(refusal: Refusal) -> SnipError {
    SnipError::conflict(refusal.to_string())
}

fn unavailable_error(unavailable: Unavailable) -> SnipError {
    match unavailable {
        Unavailable::BinaryMissing | Unavailable::NotARepository => {
            SnipError::conflict(Refusal::Unavailable.to_string())
        }
        Unavailable::ProbeFailed { message } => {
            SnipError::conflict(format!("{}: {message}", Refusal::ProbeFailed))
        }
    }
}

#[cfg(test)]
mod write_tests {
    use time::{Date, Month};

    use super::*;

    fn clean_status() -> Status {
        Status {
            branch: Branch::Named {
                name: "main".to_owned(),
            },
            upstream: Some("origin/main".to_owned()),
            ahead: 1,
            behind: 0,
            staged: 0,
            unstaged: 1,
            untracked: 2,
            conflicted: Vec::new(),
            state: RepoState::Clean,
            head_oid: Some("abcdef".to_owned()),
            last_commit: None,
            upstream_commit: None,
        }
    }

    #[test]
    fn commit_preconditions_cover_every_repository_state() {
        let mut status = clean_status();
        assert_eq!(check_commit(&status), Ok(()));
        status.branch = Branch::Unborn;
        assert_eq!(check_commit(&status), Ok(()), "unborn may create HEAD");

        status = clean_status();
        status.conflicted.push("snippet.toml".to_owned());
        assert_eq!(check_commit(&status), Err(Refusal::Conflicted));

        status = clean_status();
        status.state = RepoState::Rebasing;
        assert_eq!(
            check_commit(&status),
            Err(Refusal::MidOperation(RepoState::Rebasing))
        );

        status = clean_status();
        status.branch = Branch::Detached {
            short_id: "abcdef0".to_owned(),
        };
        assert_eq!(check_commit(&status), Err(Refusal::DetachedHead));

        status = clean_status();
        status.unstaged = 0;
        status.untracked = 0;
        assert_eq!(check_commit(&status), Err(Refusal::NothingToCommit));
    }

    #[test]
    fn push_preconditions_require_an_upstream_and_unpushed_commits() {
        let mut status = clean_status();
        assert_eq!(check_push(&status), Ok(()));

        status.upstream = None;
        assert_eq!(check_push(&status), Err(Refusal::NoUpstream));
        status.upstream = Some("origin/main".to_owned());
        status.ahead = 0;
        assert_eq!(check_push(&status), Err(Refusal::NothingToPush));

        status.ahead = 1;
        status.conflicted.push("snippet.toml".to_owned());
        assert_eq!(check_push(&status), Err(Refusal::Conflicted));
        status.conflicted.clear();
        status.state = RepoState::Merging;
        assert_eq!(
            check_push(&status),
            Err(Refusal::MidOperation(RepoState::Merging))
        );
        status.state = RepoState::Clean;
        status.branch = Branch::Detached {
            short_id: "abcdef0".to_owned(),
        };
        assert_eq!(check_push(&status), Err(Refusal::DetachedHead));
    }

    #[test]
    fn pull_preconditions_follow_the_required_priority() {
        let mut status = clean_status();
        status.unstaged = 0;
        status.untracked = 0;
        status.ahead = 0;
        assert_eq!(check_pull(&status, 0), Ok(()));

        status.conflicted.push("snippet.toml".to_owned());
        status.state = RepoState::Merging;
        status.branch = Branch::Detached {
            short_id: "abcdef0".to_owned(),
        };
        status.upstream = None;
        assert_eq!(check_pull(&status, 1), Err(Refusal::Conflicted));

        status.conflicted.clear();
        assert_eq!(
            check_pull(&status, 1),
            Err(Refusal::MidOperation(RepoState::Merging))
        );

        status.state = RepoState::Clean;
        assert_eq!(check_pull(&status, 1), Err(Refusal::DetachedHead));

        status.branch = Branch::Unborn;
        assert_eq!(check_pull(&status, 1), Err(Refusal::NothingToCommit));

        status.branch = Branch::Named {
            name: "main".to_owned(),
        };
        assert_eq!(check_pull(&status, 1), Err(Refusal::NoUpstream));

        status.upstream = Some("origin/main".to_owned());
        assert_eq!(check_pull(&status, 1), Err(Refusal::DirtyWorktree));
    }

    #[test]
    fn pull_accepts_stale_zero_behind_and_checks_repository_wide_dirt() {
        let mut status = clean_status();
        status.ahead = 0;
        status.behind = 0;
        status.staged = 0;
        status.unstaged = 0;
        status.untracked = 0;

        assert_eq!(check_pull(&status, 0), Ok(()));
        assert_eq!(status.dirty_count(), 0);
        assert_eq!(check_pull(&status, 1), Err(Refusal::DirtyWorktree));
    }

    #[test]
    fn backup_accepts_committable_changes_or_pushable_commits() {
        let dirty = clean_status();
        assert_eq!(check_backup(&dirty), Ok(()));

        let mut clean_ahead = clean_status();
        clean_ahead.unstaged = 0;
        clean_ahead.untracked = 0;
        assert_eq!(check_commit(&clean_ahead), Err(Refusal::NothingToCommit));
        assert_eq!(check_push(&clean_ahead), Ok(()));
        assert_eq!(check_backup(&clean_ahead), Ok(()));

        clean_ahead.ahead = 0;
        assert_eq!(check_backup(&clean_ahead), Err(Refusal::NothingToCommit));

        let mut conflicted = clean_status();
        conflicted.conflicted.push("conflict.rs".to_owned());
        assert_eq!(check_backup(&conflicted), Err(Refusal::Conflicted));
    }

    #[test]
    fn repo_is_send_and_sync_without_status_cache_interior_mutability() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Repo>();
    }

    #[test]
    fn auto_backup_schedule_is_derived_from_git_status() {
        let now = 10_000;
        let mut status = clean_status();
        status.last_commit = Some(Commit {
            short_id: "abcdef0".to_owned(),
            timestamp: now - 60,
            subject: "previous".to_owned(),
        });

        assert!(!should_auto_backup(&status, now, 0, false));
        assert!(!should_auto_backup(&status, now, 1, true));
        assert!(should_auto_backup(&status, now, 1, false));

        status.last_commit.as_mut().unwrap().timestamp = now - 59;
        assert!(!should_auto_backup(&status, now, 1, false));
        status.last_commit.as_mut().unwrap().timestamp = now + 60;
        assert!(!should_auto_backup(&status, now, 1, false));

        status.last_commit = None;
        status.branch = Branch::Unborn;
        assert!(should_auto_backup(&status, now, 1, false));

        status = clean_status();
        status.unstaged = 0;
        status.untracked = 0;
        assert!(!should_auto_backup(&status, now, 1, false));

        status = clean_status();
        status.conflicted.push("snippet.toml".to_owned());
        assert!(!should_auto_backup(&status, now, 1, false));

        status = clean_status();
        status.state = RepoState::Rebasing;
        assert!(!should_auto_backup(&status, now, 1, false));

        status = clean_status();
        status.branch = Branch::Detached {
            short_id: "abcdef0".to_owned(),
        };
        assert!(!should_auto_backup(&status, now, 1, false));
    }

    #[test]
    fn auto_push_delegates_every_status_guard_to_push_preconditions() {
        let mut status = clean_status();
        status.unstaged = 0;
        status.untracked = 0;
        assert!(should_auto_push(&status, true, false));
        assert!(!should_auto_push(&status, false, false));
        assert!(!should_auto_push(&status, true, true));

        status.upstream = None;
        assert!(!should_auto_push(&status, true, false));
        status.upstream = Some("origin/main".to_owned());
        status.ahead = 0;
        assert!(!should_auto_push(&status, true, false));
        status.ahead = 1;
        status.conflicted.push("snippet.toml".to_owned());
        assert!(!should_auto_push(&status, true, false));
        status.conflicted.clear();
        status.state = RepoState::Rebasing;
        assert!(!should_auto_push(&status, true, false));
        status.state = RepoState::Clean;
        status.branch = Branch::Detached {
            short_id: "abcdef0".to_owned(),
        };
        assert!(!should_auto_push(&status, true, false));
    }

    #[test]
    fn availability_refusals_and_backup_message_are_stable() {
        assert!(
            unavailable_error(Unavailable::BinaryMissing)
                .to_string()
                .contains(&Refusal::Unavailable.to_string())
        );
        assert!(
            unavailable_error(Unavailable::ProbeFailed {
                message: "permission denied".to_owned()
            })
            .to_string()
            .contains(&Refusal::ProbeFailed.to_string())
        );
        let timestamp = Date::from_calendar_date(2026, Month::July, 27)
            .unwrap()
            .with_hms(14, 32, 0)
            .unwrap()
            .assume_utc();
        assert_eq!(
            backup_message_at(&clean_status(), timestamp),
            "snip backup: 2026-07-27 14:32 (3 files)"
        );
        let mut one_file = clean_status();
        one_file.untracked = 0;
        assert_eq!(
            backup_message_at(&one_file, timestamp),
            "snip backup: 2026-07-27 14:32 (1 file)"
        );
    }
}
