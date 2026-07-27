use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command as ProcessCommand, ExitStatus};

use assert_cmd::Command;
use predicates::prelude::*;
use snip::Library;
use snip::git::{self, Branch, RepoState, Unavailable};

fn git_available() -> bool {
    ProcessCommand::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn git(path: &Path, arguments: &[&str]) -> ExitStatus {
    ProcessCommand::new("git")
        .args(["-c", "init.defaultBranch=main"])
        .args(arguments)
        .current_dir(path)
        .env("GIT_CONFIG_GLOBAL", path.join(".empty-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap()
}

fn git_ok(path: &Path, arguments: &[&str]) {
    let status = git(path, arguments);
    assert!(status.success(), "git {} failed", arguments.join(" "));
}

fn init_repo(path: &Path) {
    git_ok(path, &["init"]);
    git_ok(path, &["config", "user.name", "snip CI"]);
    git_ok(path, &["config", "user.email", "ci@example.invalid"]);
}

fn commit_all(path: &Path, message: &str) {
    git_ok(path, &["add", "-A"]);
    git_ok(
        path,
        &[
            "-c",
            "user.name=snip CI",
            "-c",
            "user.email=ci@example.invalid",
            "commit",
            "-m",
            message,
        ],
    );
}

fn append(path: &Path, content: &str) {
    OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
}

fn git_stdout(path: &Path, arguments: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .args(arguments)
        .current_dir(path)
        .env("GIT_CONFIG_GLOBAL", path.join(".empty-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed",
        arguments.join(" ")
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn reports_unborn_clean_and_mixed_worktree_states() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Git 状态.sniplib");
    let library = Library::init(&root, Some("Git status")).unwrap();
    init_repo(&root);

    let repo = git::probe(library.root()).unwrap();
    let unborn = git::status(&repo).unwrap();
    assert_eq!(unborn.branch, Branch::Unborn);
    assert!(unborn.last_commit.is_none());

    git::commit(&repo, "initial library").unwrap();
    let clean = git::status(&repo).unwrap();
    assert_eq!(
        clean.branch,
        Branch::Named {
            name: "main".to_owned()
        }
    );
    assert_eq!(clean.dirty_count(), 0);
    assert_eq!(clean.state, RepoState::Clean);
    assert_eq!(
        clean
            .last_commit
            .as_ref()
            .map(|commit| commit.subject.as_str()),
        Some("initial library")
    );
    assert!(
        git::commit(&repo, "must refuse")
            .unwrap_err()
            .to_string()
            .contains("no library changes")
    );
    assert!(
        git::push(&repo)
            .unwrap_err()
            .to_string()
            .contains("no upstream")
    );

    append(&root.join("tags.toml"), "\n# staged\n");
    git_ok(&root, &["add", "tags.toml"]);
    append(&root.join("tags.toml"), "# unstaged\n");
    fs::write(root.join("snippets/new file 中文.rs"), "fn main() {}\n").unwrap();
    let mixed = git::status(&repo).unwrap();
    assert_eq!(mixed.staged, 1);
    assert_eq!(mixed.unstaged, 1);
    assert_eq!(mixed.untracked, 1);
}

#[test]
fn scopes_file_counts_to_a_library_inside_a_parent_repository() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let parent = temporary.path();
    init_repo(parent);
    fs::write(parent.join("outside.txt"), "tracked\n").unwrap();
    let root = parent.join("Nested.sniplib");
    let library = Library::init(&root, Some("Nested")).unwrap();
    commit_all(parent, "initial");

    append(&parent.join("outside.txt"), "outside dirty\n");
    git_ok(parent, &["add", "outside.txt"]);
    append(&root.join("tags.toml"), "\n# inside dirty\n");
    let repo = git::probe(library.root()).unwrap();
    assert_eq!(repo.root, fs::canonicalize(parent).unwrap());
    assert_eq!(repo.library_prefix.as_deref(), Some("Nested.sniplib"));
    let status = git::status(&repo).unwrap();
    assert_eq!(status.unstaged, 1, "outside changes must not be counted");
    git::commit(&repo, "nested backup").unwrap();
    let committed = git_stdout(parent, &["show", "--pretty=format:", "--name-only", "HEAD"]);
    assert!(committed.contains("Nested.sniplib/tags.toml"));
    assert!(!committed.lines().any(|line| line == "outside.txt"));
    assert_eq!(
        git_stdout(parent, &["diff", "--cached", "--name-only"]).trim(),
        "outside.txt",
        "staged paths outside the library must remain staged"
    );
}

#[test]
fn commits_a_nested_library_on_an_unborn_parent_branch() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let parent = temporary.path();
    init_repo(parent);
    fs::write(parent.join("outside.txt"), "outside\n").unwrap();
    git_ok(parent, &["add", "outside.txt"]);
    let root = parent.join("Nested Unborn.sniplib");
    let library = Library::init(&root, Some("Nested Unborn")).unwrap();

    let repo = git::probe(library.root()).unwrap();
    let before = git::status(&repo).unwrap();
    assert_eq!(before.branch, Branch::Unborn);
    assert_eq!(
        repo.library_prefix.as_deref(),
        Some("Nested Unborn.sniplib")
    );

    git::commit(&repo, "nested initial backup").unwrap();

    let committed = git_stdout(parent, &["show", "--pretty=format:", "--name-only", "HEAD"]);
    assert!(committed.contains("Nested Unborn.sniplib/snip.toml"));
    assert!(!committed.lines().any(|line| line == "outside.txt"));
    assert_eq!(
        git_stdout(parent, &["diff", "--cached", "--name-only"]).trim(),
        "outside.txt",
        "staged paths outside the nested library must survive the initial commit"
    );
}

#[test]
fn detects_detached_head_and_a_real_merge_conflict() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Conflict.sniplib");
    let library = Library::init(&root, Some("Conflict")).unwrap();
    init_repo(&root);
    commit_all(&root, "initial");
    let repo = git::probe(library.root()).unwrap();

    git_ok(&root, &["checkout", "--detach"]);
    assert!(matches!(
        git::status(&repo).unwrap().branch,
        Branch::Detached { .. }
    ));
    append(&root.join("tags.toml"), "\n# detached\n");
    assert!(
        git::commit(&repo, "must refuse")
            .unwrap_err()
            .to_string()
            .contains("detached")
    );
    git_ok(&root, &["checkout", "--", "tags.toml"]);
    git_ok(&root, &["checkout", "main"]);

    git_ok(&root, &["checkout", "-b", "other"]);
    fs::write(root.join("tags.toml"), "[[tags]]\nname = \"other\"\n").unwrap();
    commit_all(&root, "other change");
    git_ok(&root, &["checkout", "main"]);
    fs::write(root.join("tags.toml"), "[[tags]]\nname = \"main\"\n").unwrap();
    commit_all(&root, "main change");
    assert!(!git(&root, &["merge", "other"]).success());

    let conflicted = git::status(&repo).unwrap();
    assert_eq!(conflicted.state, RepoState::Merging);
    assert_eq!(conflicted.conflicted, vec!["tags.toml"]);
    assert_eq!(conflicted.staged, 0);
    assert_eq!(conflicted.unstaged, 0);
    assert!(
        git::commit(&repo, "must refuse")
            .unwrap_err()
            .to_string()
            .contains("conflicts")
    );
}

#[test]
fn backup_pushes_to_a_local_bare_remote_and_clears_ahead() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Push.sniplib");
    let bare = temporary.path().join("origin.git");
    let library = Library::init(&root, Some("Push")).unwrap();
    init_repo(&root);
    let repo = git::probe(library.root()).unwrap();
    git::commit(&repo, "initial").unwrap();
    let local_only = git::backup(&repo, "unused").unwrap();
    assert!(!local_only.committed);
    assert!(!local_only.pushed);
    assert_eq!(
        local_only.message,
        "backup is committed locally; set an upstream to enable push"
    );
    git_ok(
        temporary.path(),
        &["init", "--bare", bare.to_str().unwrap()],
    );
    git_ok(&root, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(&root, &["push", "-u", "origin", "main"]);

    append(&root.join("tags.toml"), "\n# backed up\n");
    let before = git::status(&repo).unwrap();
    let message = git::backup_message(&before);
    let outcome = git::backup(&repo, &message).unwrap();
    assert!(outcome.committed);
    assert!(outcome.pushed);
    assert_eq!(git::status(&repo).unwrap().ahead, 0);
    assert!(git_stdout(&bare, &["log", "-1", "--format=%s"]).starts_with("snip backup:"));

    append(&root.join("tags.toml"), "# interval commit\n");
    git::commit(&repo, "interval auto commit").unwrap();
    let status = git::status(&repo).unwrap();
    assert_eq!(status.dirty_count(), 0);
    assert_eq!(status.ahead, 1);
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "backup",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"committed\": false"))
        .stdout(predicate::str::contains("\"pushed\": true"))
        .stdout(predicate::str::contains("\"message\"").not());
    assert_eq!(git::status(&repo).unwrap().ahead, 0);
    assert_eq!(
        git_stdout(&bare, &["log", "-1", "--format=%s"]).trim(),
        "interval auto commit"
    );

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "backup",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"committed\": false"))
        .stdout(predicate::str::contains("\"pushed\": false"))
        .stdout(predicate::str::contains(
            "\"outcome\": \"backup is already up to date\"",
        ))
        .stdout(predicate::str::contains("\"message\"").not());

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", root.to_str().unwrap(), "git", "backup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("backup is already up to date"))
        .stdout(predicate::str::contains("message:").not());

    append(&root.join("tags.toml"), "# explicit push\n");
    git::commit(&repo, "explicit push retry").unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "push",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"action\": \"push\""))
        .stdout(predicate::str::contains("\"committed\": false"))
        .stdout(predicate::str::contains("\"pushed\": true"))
        .stdout(predicate::str::contains("\"message\"").not());
    assert_eq!(git::status(&repo).unwrap().ahead, 0);
    assert_eq!(
        git_stdout(&bare, &["log", "-1", "--format=%s"]).trim(),
        "explicit push retry"
    );
}

#[test]
fn backup_reports_when_commit_succeeds_but_push_fails() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Partial Backup.sniplib");
    let bare = temporary.path().join("origin.git");
    let missing = temporary.path().join("missing.git");
    let library = Library::init(&root, Some("Partial Backup")).unwrap();
    init_repo(&root);
    let repo = git::probe(library.root()).unwrap();
    git::commit(&repo, "initial").unwrap();
    git_ok(
        temporary.path(),
        &["init", "--bare", bare.to_str().unwrap()],
    );
    git_ok(&root, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(&root, &["push", "-u", "origin", "main"]);
    git_ok(
        &root,
        &["remote", "set-url", "origin", missing.to_str().unwrap()],
    );

    append(&root.join("tags.toml"), "\n# committed before push\n");
    let error = git::backup(&repo, "backup before failed push").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("backup was committed, but push failed")
    );
    assert!(
        !error.to_string().contains("pull in your terminal"),
        "an unavailable remote must not get a non-fast-forward hint"
    );
    assert_eq!(
        git_stdout(&root, &["log", "-1", "--format=%s"]).trim(),
        "backup before failed push"
    );
    assert_eq!(git::status(&repo).unwrap().ahead, 1);
}

#[test]
fn commit_uses_the_library_lock_for_its_snapshot() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Locked.sniplib");
    let library = Library::init(&root, Some("Locked")).unwrap();
    init_repo(&root);
    let repo = git::probe(library.root()).unwrap();
    let library_lock = library.lock().unwrap();
    let error = git::commit(&repo, "must refuse").unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("library is locked by another process:")
    );
    drop(library_lock);
}

#[test]
fn cli_status_is_structured_and_unavailable_is_not_an_error() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Cli.sniplib");
    Library::init(&root, Some("CLI")).unwrap();

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "status",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"available\": false"))
        .stdout(predicate::str::contains("\"not_a_repository\""));

    init_repo(&root);
    commit_all(&root, "initial");
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "status",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"available\": true"))
        .stdout(predicate::str::contains("\"name\": \"main\""));

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", root.to_str().unwrap(), "git", "commit"])
        .assert()
        .code(4);

    assert!(matches!(
        git::probe(temporary.path().join("missing").as_path()),
        Err(Unavailable::ProbeFailed { .. })
    ));
}

#[test]
fn cli_restores_init_git_and_noninteractive_commit() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Init Git.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--output", "json", "init", root.to_str().unwrap(), "--git"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"git_initialized\": true"));
    git_ok(&root, &["config", "user.name", "snip CI"]);
    git_ok(&root, &["config", "user.email", "ci@example.invalid"]);

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "commit",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("snip backup:"))
        .stdout(predicate::str::contains("\"action\": \"commit\""));

    append(&root.join("tags.toml"), "\n# custom\n");
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "git",
            "commit",
            "-m",
            "custom backup",
        ])
        .assert()
        .success();
    assert_eq!(
        git_stdout(&root, &["log", "-1", "--format=%s"]).trim(),
        "custom backup"
    );
}

#[test]
fn cli_reports_a_missing_git_binary_as_read_only_availability() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("No Git.sniplib");
    Library::init(&root, Some("No Git")).unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env("PATH", temporary.path())
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "status",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"binary_missing\""));
}

#[cfg(unix)]
#[test]
fn cli_distinguishes_an_unexecutable_git_from_a_non_repository() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Broken Git.sniplib");
    Library::init(&root, Some("Broken Git")).unwrap();
    let bin = temporary.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let fake_git = bin.join("git");
    fs::write(&fake_git, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o644)).unwrap();

    Command::cargo_bin("snip")
        .unwrap()
        .env("PATH", &bin)
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "status",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"probe_failed\""))
        .stdout(predicate::str::contains("cannot run git"));
}
