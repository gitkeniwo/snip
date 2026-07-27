use serde::Serialize;
use snip::Library;
use snip::error::{Result, SnipError};
use snip::git::{self, Status, Unavailable};
use snip::importer::import_snippetslab;
use snip::service::{doctor, organize};
use std::path::Path;

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
        GitCommand::Status => command_git_status(library, output),
        GitCommand::Commit { message } => command_git_commit(library, message.as_deref(), output),
        GitCommand::Backup => command_git_backup(library, output),
        GitCommand::Push => command_git_push(library, output),
    }
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
