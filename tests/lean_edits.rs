//! Editing part of a large fragment without reading or echoing all of it:
//! `--lines` splices a range, `--brief` keeps the text out of the result.

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn run(library: &Path, arguments: &[&str], stdin: &str) -> std::process::Output {
    Command::cargo_bin("snip")
        .unwrap()
        .arg("--library")
        .arg(library)
        .args(["--output", "json"])
        .args(arguments)
        .write_stdin(stdin)
        .output()
        .unwrap()
}

fn json(library: &Path, arguments: &[&str], stdin: &str) -> (Value, usize) {
    let output = run(library, arguments, stdin);
    assert!(
        output.status.success(),
        "snip {} failed:\n{}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        serde_json::from_slice(&output.stdout).unwrap(),
        output.stdout.len(),
    )
}

fn cat(library: &Path, arguments: &[&str]) -> String {
    let output = Command::cargo_bin("snip")
        .unwrap()
        .arg("--library")
        .arg(library)
        .arg("cat")
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn big_text() -> String {
    (1..=1000)
        .map(|line| format!("line {line:04} of the big fragment\n"))
        .collect()
}

fn fixture() -> (tempfile::TempDir, PathBuf, String) {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Lean.sniplib");
    json(&library, &["init", library.to_str().unwrap()], "");
    let (created, _) = json(
        &library,
        &[
            "create",
            "--title",
            "Big",
            "--language",
            "text",
            "--content-file",
            "-",
        ],
        &big_text(),
    );
    json(
        &library,
        &[
            "fragment",
            "add",
            "Big",
            "--title",
            "Second",
            "--content-file",
            "-",
        ],
        &big_text(),
    );
    let id = created["snippet"]["id"].as_str().unwrap().to_owned();
    (temporary, library, id)
}

fn fingerprint(library: &Path, id: &str) -> String {
    json(library, &["show", id, "--outline"], "").0["fingerprint"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn edit_lines_replaces_only_that_range() {
    let (_temporary, library, id) = fixture();
    let hash = fingerprint(&library, &id);
    let (result, size) = json(
        &library,
        &[
            "edit",
            &id,
            "--fragment",
            "1",
            "--lines",
            "500:501",
            "--content",
            "changed 500\nchanged 501",
            "--if-hash",
            &hash,
            "--brief",
        ],
        "",
    );
    let mut expected = big_text().lines().map(str::to_owned).collect::<Vec<_>>();
    expected[499] = "changed 500".to_owned();
    expected[500] = "changed 501".to_owned();
    let expected = expected.join("\n") + "\n";
    assert_eq!(cat(&library, &[&id, "--fragment", "1"]), expected);
    assert_eq!(cat(&library, &[&id, "--fragment", "2"]), big_text());

    assert!(size < 1_000, "brief mutation printed {size} bytes");
    assert_eq!(result["snippet"]["id"], id);
    assert!(result["snippet"].get("loaded_fragments").is_none());
    assert_eq!(result["changes"]["fields"][0], "fragments[1].content");
    assert_ne!(result["changes"]["new_fingerprint"], hash);
}

#[test]
fn writing_back_what_cat_lines_printed_changes_nothing() {
    let (_temporary, library, id) = fixture();
    let before = fingerprint(&library, &id);
    for range in ["1", "10:20", "995:", "990:2000"] {
        let slice = cat(&library, &[&id, "--lines", range]);
        json(
            &library,
            &[
                "edit",
                &id,
                "--lines",
                range,
                "--content-file",
                "-",
                "--brief",
            ],
            &slice,
        );
    }
    assert_eq!(fingerprint(&library, &id), before);
}

#[test]
fn lines_pin_the_fingerprint_and_need_content() {
    let (_temporary, library, id) = fixture();
    let stale = fingerprint(&library, &id);
    json(&library, &["edit", &id, "--title", "Big renamed"], "");

    let output = run(
        &library,
        &[
            "edit",
            &id,
            "--lines",
            "3",
            "--content",
            "x",
            "--if-hash",
            &stale,
        ],
        "",
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected"));

    let output = run(&library, &["edit", &id, "--lines", "3"], "");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--content"));

    let output = run(
        &library,
        &["edit", &id, "--lines", "1001", "--content", "x"],
        "",
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(cat(&library, &[&id]), big_text());
}

#[test]
fn fragment_edit_takes_lines_and_brief() {
    let (_temporary, library, id) = fixture();
    let (result, size) = json(
        &library,
        &[
            "fragment",
            "edit",
            &id,
            "2",
            "--lines",
            "1:999",
            "--content",
            "",
            "--brief",
        ],
        "",
    );
    assert!(size < 1_000, "brief mutation printed {size} bytes");
    assert_eq!(result["snippet"]["fragments"], 2);
    assert_eq!(
        cat(&library, &[&id, "--fragment", "2"]),
        "line 1000 of the big fragment\n"
    );

    let (full, size) = json(
        &library,
        &["fragment", "edit", &id, "2", "--title", "Tail"],
        "",
    );
    assert!(
        size > 30_000,
        "default mutation output keeps the full snippet"
    );
    assert!(full["snippet"]["loaded_fragments"].is_array());
}

#[test]
fn create_brief_omits_the_text() {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Lean.sniplib");
    json(&library, &["init", library.to_str().unwrap()], "");
    let (result, size) = json(
        &library,
        &["create", "--title", "Big", "--content-file", "-", "--brief"],
        &big_text(),
    );
    assert!(size < 1_000, "brief create printed {size} bytes");
    assert_eq!(result["snippet"]["title"], "Big");
    assert!(result["changes"].is_null());
}

#[test]
fn edit_lines_accepts_a_citation_selector() {
    let (_temporary, library, id) = fixture();
    let citation = json(&library, &["list", "--brief"], "").0[0]["citation"]
        .as_str()
        .unwrap()
        .to_owned();
    let (result, _) = json(
        &library,
        &[
            "edit",
            &format!("{citation}#2"),
            "--lines",
            "1",
            "--content",
            "first",
            "--brief",
        ],
        "",
    );
    assert_eq!(result["changes"]["fields"][0], "fragments[2].content");
    assert!(cat(&library, &[&id, "--fragment", "2"]).starts_with("first\nline 0002"));
    assert_eq!(cat(&library, &[&id, "--fragment", "1"]), big_text());
}
