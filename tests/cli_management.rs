use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn command(library: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::cargo_bin("snip").unwrap();
    command
        .arg("--library")
        .arg(library)
        .args(["--output", "json"])
        .args(arguments);
    command
}

fn json(library: &Path, arguments: &[&str]) -> Value {
    let output = command(library, arguments).output().unwrap();
    assert!(
        output.status.success(),
        "snip {} failed:\n{}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn snippet_id(value: &Value) -> String {
    value["snippet"]["id"].as_str().unwrap().to_owned()
}

#[test]
fn cli_manages_snippet_fragments_folders_tags_and_trash() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Manage.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();

    let content = temporary.path().join("content.sh");
    let note = temporary.path().join("note.md");
    let readme = temporary.path().join("README.md");
    let second = temporary.path().join("second.md");
    fs::write(&content, "echo original\n").unwrap();
    fs::write(&note, "# First note\n").unwrap();
    fs::write(&readme, "# Overview\n").unwrap();
    fs::write(&second, "# Second fragment\n").unwrap();

    let created = json(
        &library,
        &[
            "create",
            "--title",
            "Manage me",
            "--folder",
            "Work/CLI",
            "--tag",
            "old",
            "--tag",
            "keep",
            "--language",
            "bash",
            "--content-file",
            content.to_str().unwrap(),
            "--note-file",
            note.to_str().unwrap(),
            "--readme-file",
            readme.to_str().unwrap(),
            "--pin",
        ],
    );
    let id = snippet_id(&created);
    assert_eq!(created["snippet"]["pinned"], true);

    let edited = json(
        &library,
        &[
            "edit",
            &id,
            "--title",
            "Managed",
            "--folder",
            "Work/CLI/Updated",
            "--tag",
            "fresh",
            "--unpin",
            "--content-file",
            content.to_str().unwrap(),
        ],
    );
    assert_eq!(edited["snippet"]["title"], "Managed");
    assert_eq!(edited["snippet"]["pinned"], false);

    let with_second = json(
        &library,
        &[
            "fragment",
            "add",
            &id,
            "--title",
            "Second",
            "--language",
            "markdown",
            "--content-file",
            second.to_str().unwrap(),
            "--note-file",
            note.to_str().unwrap(),
        ],
    );
    assert_eq!(
        with_second["snippet"]["loaded_fragments"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    json(
        &library,
        &[
            "fragment",
            "edit",
            &id,
            "2",
            "--title",
            "Second edited",
            "--language",
            "text",
            "--clear-note",
        ],
    );
    json(
        &library,
        &["fragment", "reorder", &id, "2", "--position", "1"],
    );
    let reordered = json(&library, &["show", &id]);
    assert_eq!(reordered["loaded_fragments"][0]["title"], "Second edited");
    json(&library, &["fragment", "remove", &id, "2"]);
    assert_eq!(
        json(&library, &["show", &id])["loaded_fragments"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    json(&library, &["folder", "create", "Temporary"]);
    json(
        &library,
        &["folder", "move", "Temporary", "Archive/Temporary"],
    );
    json(&library, &["folder", "rename", "Archive/Temporary", "Done"]);
    json(&library, &["folder", "delete", "Archive/Done"]);
    assert!(
        json(&library, &["folder", "list"])
            .as_array()
            .unwrap()
            .iter()
            .all(|folder| folder != "Archive/Done")
    );

    json(&library, &["tag", "rename", "fresh", "release"]);
    assert!(json(&library, &["list"]).to_string().contains("release"));
    json(&library, &["tag", "delete", "release"]);
    assert!(
        !json(&library, &["show", &id])
            .to_string()
            .contains("release")
    );

    let preview = json(&library, &["organize", "--dry-run"]);
    assert!(preview.is_array());
    json(&library, &["doctor"]);

    let deleted = json(&library, &["delete", &id]);
    let entry_id = deleted["entry_id"].as_str().unwrap().to_owned();
    assert_eq!(json(&library, &["trash"]).as_array().unwrap().len(), 1);
    json(&library, &["restore", &entry_id]);
    assert_eq!(json(&library, &["list"]).as_array().unwrap().len(), 1);
    let deleted_again = json(&library, &["delete", &id]);
    let second_entry_id = deleted_again["entry_id"].as_str().unwrap().to_owned();
    command(&library, &["purge", &second_entry_id])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--yes"));
    json(&library, &["purge", &second_entry_id, "--yes"]);
    assert!(json(&library, &["trash"]).as_array().unwrap().is_empty());
}

#[test]
fn cli_reports_selector_ambiguity_and_jsonl_records() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Selectors.sniplib");
    Command::cargo_bin("snip")
        .unwrap()
        .args(["init", library.to_str().unwrap()])
        .assert()
        .success();
    for folder in ["One", "Two"] {
        command(
            &library,
            &["create", "--title", "Duplicate", "--folder", folder],
        )
        .assert()
        .success();
    }
    command(&library, &["show", "Duplicate"])
        .assert()
        .code(3)
        .stderr(predicates::str::contains("ambiguous"));

    let output = Command::cargo_bin("snip")
        .unwrap()
        .args([
            "--library",
            library.to_str().unwrap(),
            "--output",
            "jsonl",
            "list",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 2);
}

#[test]
fn list_sort_and_open_share_the_tui_vocabulary() {
    let temporary = tempfile::tempdir_in(".").unwrap();
    let library = temporary.path().join("Sort.sniplib");
    json(&library, &["init", library.to_str().unwrap()]);

    for title in ["Charlie", "Alpha", "Bravo"] {
        json(
            &library,
            &["create", "--title", title, "--language", "text"],
        );
    }
    // Pinning must win over every sort mode, exactly as the TUI list does.
    json(&library, &["edit", "Charlie", "--pin"]);

    let titles = |mode: &str| -> Vec<String> {
        let value = json(&library, &["list", "--sort", mode]);
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["title"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(titles("title"), ["Charlie", "Alpha", "Bravo"]);
    assert_eq!(titles("created")[0], "Charlie");
    assert_eq!(titles("modified")[0], "Charlie");

    let rejected = command(&library, &["list", "--sort", "nonsense"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());

    // `snip open` is the CLI counterpart of the TUI's `v` key: same target flags as
    // `snip path`, but the resolved path is handed to an application.
    let expected = command(&library, &["path", "Alpha"]).output().unwrap();
    let expected = String::from_utf8_lossy(&expected.stdout).trim().to_owned();
    let opened = json(&library, &["open", "Alpha", "--app", "true"]);
    assert_eq!(opened["opened"].as_str().unwrap(), expected);
    assert_eq!(opened["app"].as_str().unwrap(), "true");

    let missing_app = command(&library, &["open", "Alpha", "--app", "snip-no-such-binary"])
        .output()
        .unwrap();
    assert!(!missing_app.status.success());
}
