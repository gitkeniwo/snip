use snip::Library;
use snip::error::{Result, SnipError};
use snip::importer::import_snippetslab;
use snip::service::{doctor, organize};
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

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

pub fn command_git(library: &Library, args: &GitArgs) -> Result<()> {
    match &args.command {
        GitCommand::Status => stream_git(library, &["status", "--short", "--", "."]),
        GitCommand::Diff => stream_git(library, &["diff", "--", "."]),
        GitCommand::Log { limit } => stream_git(
            library,
            &["log", "--oneline", &format!("-{limit}"), "--", "."],
        ),
        GitCommand::Commit { message } => {
            let top = git_output(library, &["rev-parse", "--show-toplevel"])?;
            let top = fs::canonicalize(top.trim()).map_err(|error| {
                SnipError::io(format!("cannot resolve Git root {:?}: {error}", top.trim()))
            })?;
            if top != library.root() {
                return Err(SnipError::conflict(
                    "snip git commit is allowed only when the library root is the Git root; use Git directly for nested libraries",
                ));
            }
            stream_git(
                library,
                &[
                    "add",
                    "--",
                    "snip.toml",
                    "tags.toml",
                    "snippets",
                    "trash",
                    ".gitignore",
                ],
            )?;
            stream_git(library, &["commit", "-m", message])
        }
    }
}

pub fn run_process(command: &mut ProcessCommand, label: &str) -> Result<()> {
    let status = command
        .status()
        .map_err(|error| SnipError::io(format!("cannot run {label}: {error}")))?;
    if !status.success() {
        return Err(SnipError::io(format!(
            "{label} exited with status {status}"
        )));
    }
    Ok(())
}

fn stream_git(library: &Library, args: &[&str]) -> Result<()> {
    run_process(
        ProcessCommand::new("git")
            .args(args)
            .current_dir(library.root()),
        "git",
    )
}

fn git_output(library: &Library, args: &[&str]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(library.root())
        .output()
        .map_err(|error| SnipError::io(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(SnipError::io(format!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| SnipError::validation(format!("git output is not UTF-8: {error}")))
}
