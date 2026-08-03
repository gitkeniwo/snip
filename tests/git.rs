use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command as ProcessCommand, ExitStatus};

use assert_cmd::Command;
use predicates::prelude::*;
use snip::Library;
use snip::config::AppConfig;
use snip::git::{self, Branch, RepoState, Unavailable};
use snip::service::{CreateOptions, create_snippet};

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
    git_ok(path, &["config", "core.autocrlf", "false"]);
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

fn committed_library(parent: &Path, name: &str) -> std::path::PathBuf {
    let root = parent.join(name);
    Library::init(&root, Some("Clone source")).unwrap();
    init_repo(&root);
    commit_all(&root, "initial");
    root
}

struct PullPair {
    _temporary: tempfile::TempDir,
    a: std::path::PathBuf,
    b: std::path::PathBuf,
    shared_fragment: std::path::PathBuf,
}

impl PullPair {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let a = temporary.path().join("A.sniplib");
        let b = temporary.path().join("B.sniplib");
        let bare = temporary.path().join("origin.git");
        let library = Library::init(&a, Some("Pull test")).unwrap();
        let shared = create_snippet(
            &library,
            &CreateOptions {
                title: "Shared snippet".to_owned(),
                language: "text".to_owned(),
                content: "base\n".to_owned(),
                ..CreateOptions::default()
            },
        )
        .unwrap();
        let shared_fragment = shared.loaded_fragments[0]
            .absolute_path
            .strip_prefix(library.root())
            .unwrap()
            .to_path_buf();
        init_repo(&a);
        commit_all(&a, "initial");
        git_ok(
            temporary.path(),
            &["init", "--bare", bare.to_str().unwrap()],
        );
        git_ok(&a, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git_ok(&a, &["push", "-u", "origin", "main"]);
        git_ok(
            temporary.path(),
            &["clone", bare.to_str().unwrap(), b.to_str().unwrap()],
        );
        git_ok(&b, &["config", "core.autocrlf", "false"]);
        git_ok(&b, &["config", "user.name", "snip CI"]);
        git_ok(&b, &["config", "user.email", "ci@example.invalid"]);
        Library::open(&b).unwrap();
        Self {
            _temporary: temporary,
            a,
            b,
            shared_fragment,
        }
    }

    fn push_a(&self, message: &str) {
        commit_all(&self.a, message);
        git_ok(&self.a, &["push"]);
    }

    fn create_snippet(&self, root: &Path, title: &str, content: &str) {
        let library = Library::open(root).unwrap();
        create_snippet(
            &library,
            &CreateOptions {
                title: title.to_owned(),
                language: "text".to_owned(),
                content: content.to_owned(),
                ..CreateOptions::default()
            },
        )
        .unwrap();
    }
}

#[cfg(unix)]
fn executable(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
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

fn assert_no_conflict_markers(path: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            assert_no_conflict_markers(&path);
        } else {
            let bytes = fs::read(&path).unwrap();
            assert!(
                !bytes.windows(7).any(|window| window == b"<<<<<<<"),
                "conflict marker remained in {}",
                path.display()
            );
        }
    }
}

#[test]
fn cloned_library_recreates_untracked_runtime_directories_on_open() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Source.sniplib");
    let clone = temporary.path().join("Clone.sniplib");
    let library = Library::init(&root, Some("Source")).unwrap();
    fs::write(library.snippets_dir().join("tracked.txt"), "tracked\n").unwrap();
    init_repo(&root);
    commit_all(&root, "initial");
    git_ok(
        temporary.path(),
        &["clone", root.to_str().unwrap(), clone.to_str().unwrap()],
    );

    assert!(!clone.join(".snip").exists());
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            clone.to_str().unwrap(),
            "--output",
            "json",
            "info",
        ])
        .assert()
        .success();
    assert!(clone.join(".snip/locks").is_dir());
}

#[test]
fn cloned_library_recreates_empty_content_directories_on_open() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Empty Source.sniplib");
    let clone = temporary.path().join("Empty Clone.sniplib");
    Library::init(&root, Some("Empty Source")).unwrap();
    init_repo(&root);
    commit_all(&root, "initial");
    git_ok(
        temporary.path(),
        &["clone", root.to_str().unwrap(), clone.to_str().unwrap()],
    );

    assert!(!clone.join("snippets").exists());
    assert!(!clone.join("trash").exists());
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", clone.to_str().unwrap(), "info"])
        .assert()
        .success();
    assert!(clone.join("snippets").is_dir());
    assert!(clone.join("trash").is_dir());
}

#[test]
fn cli_git_clone_restores_a_library_and_prints_next_steps() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "Source library.sniplib");
    let destination = temporary.path().join("Restored library.sniplib");
    let canonical_destination = fs::canonicalize(temporary.path())
        .unwrap()
        .join("Restored library.sniplib");
    let config_home = temporary.path().join("config");
    let expected = format!(
        "\n  cloned    {}\n  default   none set\n\n  browse it now          snip --library {}\n  make it your default   snip config set default-library {}\n\n",
        canonical_destination.display(),
        canonical_destination.display(),
        canonical_destination.display()
    );

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args([
            "git",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(expected);
    assert!(destination.join("snip.toml").is_file());
    assert!(destination.join(".snip/locks").is_dir());
}

#[test]
fn cli_git_clone_json_has_the_stable_contract() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "JSON source.sniplib");
    let destination = temporary.path().join("JSON clone.sniplib");
    let output = Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args([
            "--output",
            "json",
            "git",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["path"],
        fs::canonicalize(&destination).unwrap().to_str().unwrap()
    );
    assert!(value["id"].is_string());
    assert_eq!(value["name"], "Clone source");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["remote"], source.to_str().unwrap());
    assert_eq!(value["via"], "git");
    assert_eq!(value["default_library_set"], false);
}

#[test]
fn cli_git_clone_can_set_the_default_library() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "Default source.sniplib");
    let destination = temporary.path().join("Default clone.sniplib");
    let canonical_destination = fs::canonicalize(temporary.path())
        .unwrap()
        .join("Default clone.sniplib");
    let config_home = temporary.path().join("config");
    let expected = format!(
        "\n  cloned    {}\n  default   {} [updated]\n\n  open it                snip\n\n",
        canonical_destination.display(),
        canonical_destination.display()
    );

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args([
            "git",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
            "--set-default",
        ])
        .assert()
        .success()
        .stdout(expected);
    let config = AppConfig::load_from(&config_home.join("snip/config.toml")).unwrap();
    assert_eq!(
        config.default_library.as_deref(),
        Some(canonical_destination.as_path())
    );
}

#[test]
fn cli_git_clone_preserves_an_existing_default_without_the_flag() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "Other source.sniplib");
    let destination = temporary.path().join("Other clone.sniplib");
    let old_default = temporary.path().join("Old.sniplib");
    let config_home = temporary.path().join("config");
    let config = AppConfig {
        default_library: Some(old_default.clone()),
        ..AppConfig::default()
    };
    config
        .save_to(&config_home.join("snip/config.toml"))
        .unwrap();
    let canonical_destination = fs::canonicalize(temporary.path())
        .unwrap()
        .join("Other clone.sniplib");
    let expected = format!(
        "\n  cloned    {}\n  default   {} [unchanged]\n\n  browse it now          snip --library {}\n  make it your default   snip config set default-library {}\n\n",
        canonical_destination.display(),
        old_default.display(),
        canonical_destination.display(),
        canonical_destination.display()
    );

    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", &config_home)
        .args([
            "git",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn cli_git_clone_derives_the_default_destination_from_the_remote_basename() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "Derived.git");
    let home = temporary.path().join("home");
    fs::create_dir(&home).unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args(["--output", "json", "git", "clone", source.to_str().unwrap()])
        .assert()
        .success();
    assert!(home.join("Derived.sniplib/snip.toml").is_file());
}

#[test]
fn cli_git_clone_uses_userprofile_when_home_is_not_set() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "UserProfile.git");
    let userprofile = temporary.path().join("userprofile");
    fs::create_dir(&userprofile).unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env_remove("HOME")
        .env("USERPROFILE", &userprofile)
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args(["--output", "json", "git", "clone", source.to_str().unwrap()])
        .assert()
        .success();
    assert!(userprofile.join("UserProfile.sniplib/snip.toml").is_file());
}

#[test]
fn cli_git_clone_refuses_a_nonempty_destination_without_touching_it() {
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("occupied");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("keep.txt"), "keep\n").unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args(["git", "clone", "remote", destination.to_str().unwrap()])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("destination is not empty"))
        .stderr(predicate::str::contains(
            "pass a different path, or remove the directory first",
        ));
    assert_eq!(
        fs::read_to_string(destination.join("keep.txt")).unwrap(),
        "keep\n"
    );
}

#[test]
fn cli_git_clone_reports_destination_derivation_and_home_errors() {
    let temporary = tempfile::tempdir().unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args(["git", "clone", "/"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "cannot derive a destination from remote /",
        ))
        .stderr(predicate::str::contains(
            "pass an explicit path: snip git clone <remote> <path>",
        ));
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .args(["git", "clone", "owner/repo"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot locate home directory"));
}

#[test]
fn cli_git_clone_reports_a_missing_git_binary() {
    let temporary = tempfile::tempdir().unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .env("PATH", temporary.path())
        .args([
            "git",
            "clone",
            "remote",
            temporary.path().join("clone").to_str().unwrap(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("git not found in PATH"));
}

#[test]
fn cli_git_clone_removes_a_new_destination_when_repository_is_not_a_library() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("ordinary");
    fs::create_dir(&source).unwrap();
    init_repo(&source);
    fs::write(source.join("README.md"), "ordinary\n").unwrap();
    commit_all(&source, "initial");
    let destination = temporary.path().join("clone");
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args([
            "git",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ])
        .assert()
        .code(5)
        .stderr(predicate::str::contains(format!(
            "{} is not a snip library: no snip.toml at the repository root",
            source.display()
        )));
    assert!(!destination.exists());
}

#[test]
fn cli_git_clone_keeps_a_preexisting_empty_destination_after_failure() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("empty");
    fs::create_dir(&destination).unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .args([
            "git",
            "clone",
            temporary.path().join("missing").to_str().unwrap(),
            destination.to_str().unwrap(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("git clone failed"));
    assert!(destination.is_dir());
}

#[cfg(unix)]
#[test]
fn cli_git_clone_selects_failure_hints_in_order() {
    let cases = [
        (
            "Host key verification failed\nPermission denied (publickey)",
            "the host key is not in known_hosts yet; connect once manually, for example: ssh -T git@github.com",
        ),
        (
            "Permission denied (publickey)",
            "no usable SSH key was found; add one with ssh-add, or retry with --gh",
        ),
        (
            "fatal: could not read Username for 'https://github.com'",
            "this remote needs credentials; set up a credential helper with gh auth setup-git, or retry with --gh",
        ),
        (
            "generic failure",
            "looks like a GitHub slug — retry with --gh",
        ),
    ];
    for (index, (stderr, hint)) in cases.into_iter().enumerate() {
        let temporary = tempfile::tempdir().unwrap();
        let bin = temporary.path().join("bin");
        fs::create_dir(&bin).unwrap();
        executable(
            &bin.join("git"),
            &format!("#!/bin/sh\nprintf '%s\\n' {stderr:?} >&2\nexit 1\n"),
        );
        Command::cargo_bin("snip")
            .unwrap()
            .env("XDG_CONFIG_HOME", temporary.path().join("config"))
            .env("HOME", temporary.path())
            .env("PATH", &bin)
            .args(["git", "clone", "owner/repo"])
            .assert()
            .code(1)
            .stderr(predicate::str::contains("git clone failed"))
            .stderr(predicate::str::contains(hint));
        assert!(!temporary.path().join(format!("repo-{index}")).exists());
    }
}

#[cfg(unix)]
#[test]
fn cli_git_clone_does_not_suggest_gh_when_gh_already_failed() {
    let temporary = tempfile::tempdir().unwrap();
    let stub = temporary.path().join("gh");
    executable(
        &stub,
        "#!/bin/sh\nprintf '%s\\n' 'Permission denied (publickey)' >&2\nexit 1\n",
    );
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .env("SNIP_GH_BIN", &stub)
        .args([
            "git",
            "clone",
            "owner/repo",
            temporary.path().join("clone").to_str().unwrap(),
            "--gh",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("gh repo clone failed"))
        .stderr(predicate::str::contains("retry with --gh").not());
}

#[cfg(unix)]
#[test]
fn cli_git_clone_uses_gh_with_the_exact_arguments() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let source = committed_library(temporary.path(), "GH source.sniplib");
    let destination = temporary.path().join("GH clone.sniplib");
    let argv = temporary.path().join("argv");
    let stub = temporary.path().join("gh");
    executable(
        &stub,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$GH_STUB_ARGV\"\ncp -R \"$3\" \"$4\"\n",
    );
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .env("SNIP_GH_BIN", &stub)
        .env("GH_STUB_ARGV", &argv)
        .args([
            "--output",
            "json",
            "git",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
            "--gh",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"via\": \"gh\""));
    assert_eq!(
        fs::read_to_string(argv)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        [
            "repo",
            "clone",
            source.to_str().unwrap(),
            destination.to_str().unwrap(),
        ]
    );
}

#[cfg(unix)]
#[test]
fn cli_git_clone_classifies_gh_availability_errors() {
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("clone");
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .env("SNIP_GH_BIN", temporary.path().join("missing-gh"))
        .args([
            "git",
            "clone",
            "owner/repo",
            destination.to_str().unwrap(),
            "--gh",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("gh not found in PATH"))
        .stderr(predicate::str::contains(
            "install the GitHub CLI from https://cli.github.com, or drop --gh and pass a full https or ssh remote",
        ));

    let stub = temporary.path().join("gh");
    executable(
        &stub,
        "#!/bin/sh\nprintf '%s\\n' 'not logged into any GitHub hosts; run gh auth login' >&2\nexit 1\n",
    );
    Command::cargo_bin("snip")
        .unwrap()
        .env("XDG_CONFIG_HOME", temporary.path().join("config"))
        .env("SNIP_GH_BIN", &stub)
        .args([
            "git",
            "clone",
            "owner/repo",
            destination.to_str().unwrap(),
            "--gh",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("gh is not authenticated"))
        .stderr(predicate::str::contains("run: gh auth login"));
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
    let tracked = git::status(&repo).unwrap();
    assert_eq!(
        tracked.last_commit.as_ref().map(|commit| &commit.short_id),
        tracked
            .upstream_commit
            .as_ref()
            .map(|commit| &commit.short_id)
    );

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
fn fetch_refreshes_remote_commit_and_behind_count_without_touching_worktree() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Fetch.sniplib");
    let bare = temporary.path().join("origin.git");
    let peer = temporary.path().join("peer");
    let library = Library::init(&root, Some("Fetch")).unwrap();
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
        temporary.path(),
        &["clone", bare.to_str().unwrap(), peer.to_str().unwrap()],
    );
    git_ok(&peer, &["config", "user.name", "snip CI"]);
    git_ok(&peer, &["config", "user.email", "ci@example.invalid"]);
    fs::write(peer.join("remote-only.txt"), "remote\n").unwrap();
    commit_all(&peer, "remote change");
    git_ok(&peer, &["push"]);

    assert_eq!(git::status(&repo).unwrap().behind, 0);
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            root.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "fetch",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"action\": \"fetch\""))
        .stdout(predicate::str::contains(
            "\"outcome\": \"remote status refreshed\"",
        ));
    let refreshed = git::status(&repo).unwrap();
    assert_eq!(refreshed.behind, 1);
    assert_eq!(
        refreshed
            .upstream_commit
            .as_ref()
            .map(|commit| commit.subject.as_str()),
        Some("remote change")
    );
    assert!(!root.join("remote-only.txt").exists());
}

#[test]
fn pull_fast_forwards_and_makes_remote_snippets_readable() {
    if !git_available() {
        return;
    }
    let pair = PullPair::new();
    pair.create_snippet(&pair.a, "Remote arrival", "remote\n");
    pair.push_a("remote snippet");

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "git", "pull"])
        .assert()
        .success()
        .stdout("pulled 1 commit\nahead/behind: 0/0\n");
    let catalog = Library::open(&pair.b).unwrap().scan().unwrap();
    assert!(
        catalog
            .snippets
            .iter()
            .any(|snippet| snippet.title == "Remote arrival")
    );
}

#[test]
fn pull_reports_already_up_to_date_with_stable_json_fields() {
    if !git_available() {
        return;
    }
    let pair = PullPair::new();
    let output = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            pair.b.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "pull",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["action"], "pull");
    assert_eq!(value["pulled"], 0);
    assert_eq!(value["merged"], false);
    assert_eq!(value["outcome"], "already up to date");
    assert!(value["status"].is_object());
}

#[test]
fn pull_merges_cleanly_diverged_snippet_changes() {
    if !git_available() {
        return;
    }
    let pair = PullPair::new();
    fs::write(pair.a.join(&pair.shared_fragment), "remote side\n").unwrap();
    pair.push_a("remote edit");
    pair.create_snippet(&pair.b, "Local only", "local\n");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "git", "commit"])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            pair.b.to_str().unwrap(),
            "--output",
            "json",
            "git",
            "pull",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"pulled\": 1"))
        .stdout(predicate::str::contains("\"merged\": true"))
        .stdout(predicate::str::contains(
            "\"outcome\": \"merged 1 commit from origin/main\"",
        ));
    assert_eq!(
        fs::read_to_string(pair.b.join(&pair.shared_fragment))
            .unwrap()
            .replace("\r\n", "\n"),
        "remote side\n"
    );
    assert!(
        Library::open(&pair.b)
            .unwrap()
            .scan()
            .unwrap()
            .snippets
            .iter()
            .any(|snippet| snippet.title == "Local only")
    );
    assert!(
        !git_stdout(&pair.b, &["log", "-1", "--merges", "--format=%H"])
            .trim()
            .is_empty()
    );
}

#[test]
fn pull_ff_only_refuses_divergence_without_touching_the_worktree() {
    if !git_available() {
        return;
    }
    let pair = PullPair::new();
    fs::write(pair.a.join(&pair.shared_fragment), "remote side\n").unwrap();
    pair.push_a("remote edit");
    pair.create_snippet(&pair.b, "Local only", "local\n");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "git", "commit"])
        .assert()
        .success();
    let head = git_stdout(&pair.b, &["rev-parse", "HEAD"]);

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            pair.b.to_str().unwrap(),
            "git",
            "pull",
            "--ff-only",
        ])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "cannot fast-forward: the branch has diverged from origin/main",
        ))
        .stderr(predicate::str::contains(
            "drop --ff-only to merge, or reconcile the branches yourself with git",
        ));
    assert_eq!(git_stdout(&pair.b, &["rev-parse", "HEAD"]), head);
    assert!(git_stdout(&pair.b, &["status", "--porcelain"]).is_empty());
    assert_eq!(
        fs::read_to_string(pair.b.join(&pair.shared_fragment)).unwrap(),
        "base\n"
    );
}

#[test]
fn pull_aborts_real_conflicts_and_leaves_a_parseable_clean_library() {
    if !git_available() {
        return;
    }
    let pair = PullPair::new();
    fs::write(pair.a.join(&pair.shared_fragment), "remote side\n").unwrap();
    pair.push_a("remote conflict");
    fs::write(pair.b.join(&pair.shared_fragment), "local side\n").unwrap();
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "git", "commit"])
        .assert()
        .success();

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "git", "pull"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "pull stopped: 1 file conflict with origin/main",
        ))
        .stderr(predicate::str::contains(
            pair.shared_fragment
                .to_string_lossy()
                .replace('\\', "/"),
        ))
        .stderr(predicate::str::contains(
            "your library was left untouched; resolve it with git in the library directory: git pull",
        ));
    assert!(git_stdout(&pair.b, &["status", "--porcelain"]).is_empty());
    assert_no_conflict_markers(&pair.b);
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "list"])
        .assert()
        .success();
}

#[test]
fn pull_reports_unrelated_histories_without_a_false_merge_recovery_hint() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let local = temporary.path().join("Local.sniplib");
    let remote = temporary.path().join("Remote.sniplib");
    let bare = temporary.path().join("origin.git");

    Library::init(&local, Some("Local")).unwrap();
    init_repo(&local);
    commit_all(&local, "local initial");

    Library::init(&remote, Some("Remote")).unwrap();
    init_repo(&remote);
    commit_all(&remote, "remote initial");
    git_ok(
        temporary.path(),
        &["init", "--bare", bare.to_str().unwrap()],
    );
    git_ok(
        &remote,
        &["remote", "add", "origin", bare.to_str().unwrap()],
    );
    git_ok(&remote, &["push", "-u", "origin", "main"]);

    git_ok(&local, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(&local, &["config", "branch.main.remote", "origin"]);
    git_ok(&local, &["config", "branch.main.merge", "refs/heads/main"]);

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", local.to_str().unwrap(), "git", "pull"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "git merge failed: fatal: refusing to merge unrelated histories",
        ))
        .stderr(predicate::str::contains("could not be undone").not())
        .stderr(predicate::str::contains("recover it with").not());
    assert!(!local.join(".git/MERGE_HEAD").exists());
    assert!(git_stdout(&local, &["status", "--porcelain=v2"]).is_empty());
}

#[test]
fn pull_refuses_a_dirty_worktree_before_merging() {
    if !git_available() {
        return;
    }
    let pair = PullPair::new();
    fs::write(pair.a.join(&pair.shared_fragment), "remote side\n").unwrap();
    pair.push_a("remote edit");
    append(&pair.b.join("tags.toml"), "\n# local dirt\n");
    let head = git_stdout(&pair.b, &["rev-parse", "HEAD"]);

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", pair.b.to_str().unwrap(), "git", "pull"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "commit or stash your changes before pulling",
        ));
    assert_eq!(git_stdout(&pair.b, &["rev-parse", "HEAD"]), head);
    assert!(
        fs::read_to_string(pair.b.join("tags.toml"))
            .unwrap()
            .contains("local dirt")
    );
}

#[test]
fn pull_refuses_a_repository_without_an_upstream() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("No upstream.sniplib");
    Library::init(&root, Some("No upstream")).unwrap();
    init_repo(&root);
    commit_all(&root, "initial");

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", root.to_str().unwrap(), "git", "pull"])
        .assert()
        .code(4)
        .stderr(predicate::str::contains(
            "no upstream is configured; run git push -u origin <branch> once in your terminal",
        ));
}

#[test]
fn pruned_upstream_keeps_local_commit_metadata() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Pruned upstream.sniplib");
    let bare = temporary.path().join("origin.git");
    let library = Library::init(&root, Some("Pruned upstream")).unwrap();
    init_repo(&root);
    let repo = git::probe(library.root()).unwrap();
    git::commit(&repo, "local commit survives pruning").unwrap();
    git_ok(
        temporary.path(),
        &["init", "--bare", bare.to_str().unwrap()],
    );
    git_ok(&root, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(&root, &["push", "-u", "origin", "main"]);

    git_ok(&bare, &["update-ref", "-d", "refs/heads/main"]);
    git::fetch(&repo).unwrap();
    let pruned = git::status(&repo).unwrap();

    assert_eq!(pruned.upstream.as_deref(), Some("origin/main"));
    assert_eq!(
        pruned
            .last_commit
            .as_ref()
            .map(|commit| commit.subject.as_str()),
        Some("local commit survives pruning")
    );
    assert!(pruned.upstream_commit.is_none());
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
fn cli_git_init_creates_a_repository_and_is_idempotent() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Init.sniplib");
    Library::init(&root, Some("Init")).unwrap();
    let library = root.to_str().unwrap();

    assert!(matches!(
        git::probe(&root),
        Err(Unavailable::NotARepository)
    ));

    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library, "--output", "json", "git", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"created\": true"));
    assert!(git::probe(&root).is_ok());

    // Running it again must not fail: this is the whole reason a caller can
    // ask for a repository without probing for one first.
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library, "--output", "json", "git", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"created\": false"));

    // And the repository it made is usable, not just present.
    git_ok(&root, &["config", "user.name", "snip CI"]);
    git_ok(&root, &["config", "user.email", "ci@example.invalid"]);
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library, "git", "commit"])
        .assert()
        .success();
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
