use serde::Serialize;
use snip::Library;
use snip::config::AppConfig;
use snip::error::{Result, SnipError};
use snip::git::{self, SpawnError, Status, Unavailable};
use snip::importer::import_snippetslab;
use snip::service::{doctor, organize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use super::output::{print_record, print_records};
use crate::cli::{
    DoctorArgs, GitArgs, GitCommand, ImportArgs, ImportCommand, OrganizeArgs, OutputMode,
};

pub fn command_doctor(library: &Library, args: &DoctorArgs, output: OutputMode) -> Result<()> {
    let report = doctor(library, args.repair);
    if output == OutputMode::Human {
        println!("checked: {} snippets", report.checked);
        println!("errors: {}", report.errors.len());
        println!("warnings: {}", report.warnings.len());
        println!(
            "pending transactions: {}",
            report.pending_transactions.len()
        );
        for message in &report.repaired {
            println!("REPAIRED: {message}");
        }
        for message in &report.errors {
            println!("ERROR: {message}");
        }
        for message in &report.warnings {
            println!("WARNING: {message}");
        }
    } else {
        print_record(&report, output)?;
    }
    if !report.ok {
        return Err(SnipError::validation("library validation failed"));
    }
    Ok(())
}

pub fn command_organize(library: &Library, args: &OrganizeArgs, output: OutputMode) -> Result<()> {
    let changes = organize(library, args.dry_run)?;
    if output == OutputMode::Human {
        for change in &changes {
            println!(
                "{}{} -> {}",
                if args.dry_run {
                    "would move: "
                } else {
                    "moved: "
                },
                change
                    .old_path
                    .as_deref()
                    .unwrap_or_else(|| Path::new("-"))
                    .display(),
                change
                    .new_path
                    .as_deref()
                    .unwrap_or_else(|| Path::new("-"))
                    .display()
            );
        }
        println!("changes: {}", changes.len());
    } else {
        print_records(&changes, output)?;
    }
    Ok(())
}

pub fn command_import(args: &ImportArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        ImportCommand::Snippetslab {
            source,
            into,
            dry_run,
        } => {
            let report = import_snippetslab(source, into, *dry_run)?;
            if output == OutputMode::Human {
                println!("source: {}", report.source.display());
                println!("destination: {}", report.destination.display());
                println!("dry run: {}", report.dry_run);
                println!("snippets: {}", report.snippets);
                println!("folders: {}", report.folders);
                println!("tags: {}", report.tags);
                println!("fragments: {}", report.fragments);
                println!("notes: {}", report.notes);
                println!("attachments: {}", report.attachments);
                for item in report.normalized_tags {
                    println!("NORMALIZED TAG: {item}");
                }
                for item in report.warnings {
                    println!("WARNING: {item}");
                }
            } else {
                print_record(&report, output)?;
            }
        }
    }
    Ok(())
}

pub fn command_git(library: &Library, args: &GitArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        GitCommand::Clone { .. } => {
            unreachable!("git clone is dispatched before library resolution")
        }
        GitCommand::Status => command_git_status(library, output),
        GitCommand::Init => command_git_init(library, output),
        GitCommand::Commit { message } => command_git_commit(library, message.as_deref(), output),
        GitCommand::Backup => command_git_backup(library, output),
        GitCommand::Push => command_git_push(library, output),
        GitCommand::Fetch => command_git_fetch(library, output),
    }
}

#[derive(Serialize)]
struct GitCloneReport<'a> {
    path: &'a Path,
    id: uuid::Uuid,
    name: &'a str,
    schema_version: u32,
    remote: &'a str,
    via: &'static str,
    default_library_set: bool,
}

pub fn command_git_clone(
    remote: &str,
    path: Option<&Path>,
    gh: bool,
    set_default: bool,
    output: OutputMode,
) -> Result<()> {
    let destination = clone_destination(remote, path)?;
    let destination = if destination.is_absolute() {
        destination
    } else {
        std::env::current_dir()
            .map_err(|error| SnipError::io(format!("cannot get current directory: {error}")))?
            .join(destination)
    };
    let existed = destination.exists();
    if existed && !destination.is_dir() {
        return Err(destination_not_empty(&destination));
    }
    if existed
        && fs::read_dir(&destination)
            .map_err(|error| {
                SnipError::io(format!("cannot read {}: {error}", destination.display()))
            })?
            .next()
            .is_some()
    {
        return Err(destination_not_empty(&destination));
    }

    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| SnipError::io(format!("cannot create {}: {error}", parent.display())))?;
    let destination_text = destination.to_string_lossy();
    let clone_result = if gh {
        run_gh_clone(remote, &destination_text)
    } else {
        run_git_clone(parent, remote, &destination_text)
    };
    if let Err(error) = clone_result {
        cleanup_failed_clone(&destination, existed);
        return Err(error);
    }

    if !destination.join("snip.toml").is_file() {
        cleanup_failed_clone(&destination, existed);
        return Err(SnipError::validation(format!(
            "{remote} is not a snip library: no snip.toml at the repository root"
        ))
        .with_hint(
            "snip git clone restores libraries created by snip init; use git clone if you only want the repository",
        ));
    }
    let library = match Library::open(&destination) {
        Ok(library) => library,
        Err(error) => {
            cleanup_failed_clone(&destination, existed);
            return Err(error);
        }
    };

    let mut config = AppConfig::load()?;
    if set_default {
        config.default_library = Some(library.root().to_path_buf());
        config.save()?;
    }
    if output == OutputMode::Human {
        print_clone_human(&library, &config, set_default);
        return Ok(());
    }
    print_record(
        &GitCloneReport {
            path: library.root(),
            id: library.manifest().id,
            name: &library.manifest().name,
            schema_version: library.manifest().schema_version,
            remote,
            via: if gh { "gh" } else { "git" },
            default_library_set: set_default,
        },
        output,
    )
}

fn clone_destination(remote: &str, explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let trimmed = remote.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let separators: &[char] = if cfg!(windows) {
        &['/', ':', '\\']
    } else {
        &['/', ':']
    };
    let name = trimmed
        .rfind(separators)
        .map_or(trimmed, |index| &trimmed[index + 1..]);
    if name.is_empty() {
        return Err(
            SnipError::usage(format!("cannot derive a destination from remote {remote}"))
                .with_hint("pass an explicit path: snip git clone <remote> <path>"),
        );
    }
    let name = if name.ends_with(".sniplib") {
        name.to_owned()
    } else {
        format!("{name}.sniplib")
    };
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            SnipError::io("cannot locate home directory")
                .with_hint("pass an explicit path: snip git clone <remote> <path>")
        })?;
    Ok(PathBuf::from(home).join(name))
}

fn destination_not_empty(path: &Path) -> SnipError {
    SnipError::conflict(format!("destination is not empty: {}", path.display()))
        .with_hint("pass a different path, or remove the directory first")
}

fn run_git_clone(parent: &Path, remote: &str, destination: &str) -> Result<()> {
    let result = git::run_non_interactive(parent, &["clone", "--", remote, destination]);
    let output = match result {
        Ok(output) => output,
        Err(SpawnError::NotInstalled) => return Err(SnipError::io("git not found in PATH")),
        Err(SpawnError::Io(message)) => return Err(SnipError::io(message)),
    };
    if output.status.success() {
        return Ok(());
    }
    Err(clone_failed("git clone", remote, &output.stderr, false))
}

fn run_gh_clone(remote: &str, destination: &str) -> Result<()> {
    let binary = std::env::var("SNIP_GH_BIN")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "gh".to_owned());
    let output = ProcessCommand::new(binary)
        .args(["repo", "clone", remote, destination])
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SnipError::io("gh not found in PATH").with_hint(
                    "install the GitHub CLI from https://cli.github.com, or drop --gh and pass a full https or ssh remote",
                )
            } else {
                SnipError::io(error.to_string())
            }
        })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.contains("auth login") || stderr.contains("not logged") {
        return Err(SnipError::io("gh is not authenticated").with_hint("run: gh auth login"));
    }
    Err(clone_failed("gh repo clone", remote, &stderr, true))
}

fn clone_failed(command: &str, remote: &str, stderr: &str, gh: bool) -> SnipError {
    let error = SnipError::io(format!("{command} failed: {stderr}"));
    if stderr.contains("Host key verification failed") {
        return error.with_hint(
            "the host key is not in known_hosts yet; connect once manually, for example: ssh -T git@github.com",
        );
    }
    if !gh {
        // The remaining hints all say "retry with --gh", which is useless advice
        // when --gh is what just failed.
        if stderr.contains("Permission denied (publickey)") {
            return error.with_hint(
                "no usable SSH key was found; add one with ssh-add, or retry with --gh",
            );
        }
        if stderr.contains("could not read Username") {
            return error.with_hint(
                "this remote needs credentials; set up a credential helper with gh auth setup-git, or retry with --gh",
            );
        }
        if looks_like_slug(remote) {
            return error.with_hint("looks like a GitHub slug — retry with --gh");
        }
    }
    error
}

fn looks_like_slug(remote: &str) -> bool {
    !remote.contains("://")
        && !remote.contains('@')
        && !remote.contains(':')
        && remote.split('/').count() == 2
        && remote.split('/').all(|part| !part.is_empty())
        && !Path::new(remote).exists()
}

fn cleanup_failed_clone(destination: &Path, existed: bool) {
    if !existed && destination.is_dir() {
        let _ = fs::remove_dir_all(destination);
    }
}

fn print_clone_human(library: &Library, config: &AppConfig, set_default: bool) {
    println!("\n  {:<9} {}", "cloned", library.root().display());
    if set_default {
        println!("  {:<9} {} [updated]", "default", library.root().display());
        println!("\n  {:<22} snip\n", "open it");
        return;
    }
    match config.default_library.as_deref() {
        Some(path) => println!("  {:<9} {} [unchanged]", "default", path.display()),
        None => println!("  {:<9} none set", "default"),
    }
    println!(
        "\n  {:<22} snip --library {}",
        "browse it now",
        library.root().display()
    );
    println!(
        "  {:<22} snip config set default-library {}\n",
        "make it your default",
        library.root().display()
    );
}

#[derive(Serialize)]
struct GitStatusReport<'a> {
    available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a Unavailable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository_root: Option<&'a Path>,
    #[serde(skip_serializing_if = "Option::is_none")]
    library_prefix: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'a Status>,
}

fn command_git_status(library: &Library, output: OutputMode) -> Result<()> {
    let repo = match git::probe(library.root()) {
        Ok(repo) => repo,
        Err(reason) => {
            let report = GitStatusReport {
                available: false,
                reason: Some(&reason),
                repository_root: None,
                library_prefix: None,
                status: None,
            };
            if output == OutputMode::Human {
                println!(
                    "{}",
                    match &reason {
                        Unavailable::BinaryMissing => "git not found in PATH",
                        Unavailable::NotARepository => {
                            "this library is not inside a git repository"
                        }
                        Unavailable::ProbeFailed { message } => message,
                    }
                );
                return Ok(());
            }
            return print_record(&report, output);
        }
    };
    let status = git::status(&repo)?;
    let report = GitStatusReport {
        available: true,
        reason: None,
        repository_root: Some(&repo.root),
        library_prefix: repo.library_prefix.as_deref(),
        status: Some(&status),
    };
    if output != OutputMode::Human {
        return print_record(&report, output);
    }
    println!("repository: {}", repo.root.display());
    println!("branch: {}", branch_label(&status));
    println!(
        "changes: {} staged, {} unstaged, {} untracked",
        status.staged, status.unstaged, status.untracked
    );
    println!("ahead/behind: {}/{}", status.ahead, status.behind);
    println!(
        "upstream: {}",
        status.upstream.as_deref().unwrap_or("not configured")
    );
    if let Some(commit) = &status.last_commit {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        println!(
            "last commit: {} ({}) {}",
            commit.short_id,
            git::relative_time(commit.timestamp, now),
            commit.subject
        );
    } else {
        println!("last commit: none");
    }
    if !status.conflicted.is_empty() {
        println!("conflicts: {}", status.conflicted.len());
    }
    if status.state != git::RepoState::Clean {
        println!("repository state: {}", status.state.label());
    }
    Ok(())
}

#[derive(Serialize)]
struct GitMutationReport<'a> {
    action: &'static str,
    committed: bool,
    pushed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    outcome: &'a str,
    status: &'a Status,
}

fn command_git_commit(
    library: &Library,
    custom_message: Option<&str>,
    output: OutputMode,
) -> Result<()> {
    let repo = require_repo(library)?;
    let before = git::status(&repo)?;
    git::check_commit(&before).map_err(|refusal| SnipError::conflict(refusal.to_string()))?;
    let generated;
    let message = if let Some(message) = custom_message {
        let message = message.trim();
        if message.is_empty() {
            return Err(SnipError::usage("commit message cannot be empty"));
        }
        message
    } else {
        generated = git::backup_message(&before);
        &generated
    };
    git::commit(&repo, message)?;
    let after = git::status(&repo)?;
    print_git_mutation(
        GitMutationReport {
            action: "commit",
            committed: true,
            pushed: false,
            message: Some(message),
            outcome: "backup committed",
            status: &after,
        },
        output,
    )
}

fn command_git_backup(library: &Library, output: OutputMode) -> Result<()> {
    let repo = require_repo(library)?;
    let before = git::status(&repo)?;
    let message = git::backup_message(&before);
    let outcome = git::backup(&repo, &message)?;
    let after = git::status(&repo)?;
    print_git_mutation(
        GitMutationReport {
            action: "backup",
            committed: outcome.committed,
            pushed: outcome.pushed,
            message: outcome.committed.then_some(message.as_str()),
            outcome: &outcome.message,
            status: &after,
        },
        output,
    )
}

fn command_git_push(library: &Library, output: OutputMode) -> Result<()> {
    let repo = require_repo(library)?;
    let before = git::status(&repo)?;
    git::check_push(&before).map_err(|refusal| SnipError::conflict(refusal.to_string()))?;
    git::push(&repo)?;
    let after = git::status(&repo)?;
    print_git_mutation(
        GitMutationReport {
            action: "push",
            committed: false,
            pushed: true,
            message: None,
            outcome: "backup pushed",
            status: &after,
        },
        output,
    )
}

fn command_git_fetch(library: &Library, output: OutputMode) -> Result<()> {
    let repo = require_repo(library)?;
    git::fetch(&repo)?;
    let after = git::status(&repo)?;
    print_git_mutation(
        GitMutationReport {
            action: "fetch",
            committed: false,
            pushed: false,
            message: None,
            outcome: "remote status refreshed",
            status: &after,
        },
        output,
    )
}

#[derive(Serialize)]
struct GitInitReport<'a> {
    action: &'static str,
    created: bool,
    repository_root: &'a Path,
    outcome: &'a str,
}

fn command_git_init(library: &Library, output: OutputMode) -> Result<()> {
    // Initializing twice is not a failure, the same way `git init` itself is
    // idempotent: a caller that just wants a repository to exist should not
    // have to probe first and branch on the answer.
    let created = match git::probe(library.root()) {
        Ok(_) => false,
        Err(Unavailable::NotARepository) => {
            git::init(library.root())?;
            true
        }
        Err(Unavailable::BinaryMissing) => {
            return Err(SnipError::conflict("git not found in PATH"));
        }
        Err(Unavailable::ProbeFailed { message }) => return Err(SnipError::conflict(message)),
    };
    let repo = require_repo(library)?;
    let outcome = if created {
        "initialized a Git repository"
    } else {
        "already a Git repository"
    };
    if output == OutputMode::Human {
        println!("{outcome}: {}", repo.root.display());
        return Ok(());
    }
    print_record(
        &GitInitReport {
            action: "init",
            created,
            repository_root: &repo.root,
            outcome,
        },
        output,
    )
}

fn require_repo(library: &Library) -> Result<git::Repo> {
    git::probe(library.root()).map_err(|unavailable| match unavailable {
        Unavailable::BinaryMissing => SnipError::conflict("git not found in PATH"),
        Unavailable::NotARepository => {
            SnipError::conflict("this library is not inside a git repository")
        }
        Unavailable::ProbeFailed { message } => SnipError::conflict(message),
    })
}

fn print_git_mutation(report: GitMutationReport<'_>, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("{}", report.outcome);
        if let Some(message) = report.message {
            println!("message: {message}");
        }
        println!(
            "ahead/behind: {}/{}",
            report.status.ahead, report.status.behind
        );
        Ok(())
    } else {
        print_record(&report, output)
    }
}

fn branch_label(status: &Status) -> String {
    match &status.branch {
        git::Branch::Named { name } => name.clone(),
        git::Branch::Detached { short_id } => format!("detached@{short_id}"),
        git::Branch::Unborn => "no commits".to_owned(),
    }
}
