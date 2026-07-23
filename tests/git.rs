use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Command as ProcessCommand;

fn git(path: &std::path::Path, arguments: &[&str]) {
    let status = ProcessCommand::new("git")
        .args(arguments)
        .current_dir(path)
        .status()
        .unwrap();
    assert!(status.success(), "git {} failed", arguments.join(" "));
}

#[test]
fn git_commands_commit_a_dedicated_library_only() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Git.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap(), "--git"])
        .assert()
        .success();
    git(&library, &["config", "user.name", "snip CI"]);
    git(&library, &["config", "user.email", "ci@example.invalid"]);

    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "create",
            "--title",
            "Committed",
            "--language",
            "rust",
        ])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args(["--library", library.to_str().unwrap(), "git", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("snippets"));
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "git",
            "commit",
            "--message",
            "test: commit snippet",
        ])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "git",
            "log",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("test: commit snippet"));

    let nested = library.join("Nested.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", nested.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            nested.to_str().unwrap(),
            "git",
            "commit",
            "--message",
            "must fail",
        ])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("Git root"));
}
