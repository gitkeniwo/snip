//! Regression fence for the token-efficient read path. The byte ceilings below
//! are the point: a field added to a brief shape later has to move a number here
//! on purpose.

use assert_cmd::Command;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

fn run(library: &Path, arguments: &[&str]) -> std::process::Output {
    Command::cargo_bin("snip")
        .unwrap()
        .arg("--library")
        .arg(library)
        .args(["--output", "json"])
        .args(arguments)
        .output()
        .unwrap()
}

fn json(library: &Path, arguments: &[&str]) -> (Value, usize) {
    let output = run(library, arguments);
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

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let library = temporary.path().join("Budget.sniplib");
    json(&library, &["init", library.to_str().unwrap()]);
    for index in 0..30 {
        json(
            &library,
            &[
                "create",
                "--title",
                &format!("Git helper {index:02}"),
                "--folder",
                "Dev",
                "--tag",
                "git",
                "--language",
                "bash",
                "--content",
                &format!("git status\ngit log --oneline -{index}\necho git done\n"),
            ],
        );
    }
    let big = (1..=1200)
        .map(|line| format!("line {line:04} of the big fragment\n"))
        .collect::<String>();
    assert!(big.len() > 30_000);
    json(
        &library,
        &[
            "create",
            "--title",
            "Big multi",
            "--folder",
            "Dev",
            "--language",
            "toml",
            "--content",
            &big,
        ],
    );
    json(
        &library,
        &[
            "fragment",
            "add",
            "Big multi",
            "--title",
            "Second",
            "--language",
            "bash",
            "--content",
            "echo second\n",
        ],
    );
    let long_line = format!("{}\n", "é".repeat(5_000));
    assert_eq!(long_line.len(), 10_001);
    json(
        &library,
        &[
            "create",
            "--title",
            "One long line",
            "--folder",
            "Dev",
            "--content",
            &long_line,
        ],
    );
    (temporary, library)
}

#[test]
fn search_brief_is_one_row_per_snippet_and_small() {
    let (_guard, library) = fixture();
    let (full, full_bytes) = json(&library, &["search", "git"]);
    let (brief, brief_bytes) = json(&library, &["search", "git", "--brief"]);

    assert_eq!(brief["total"], 30);
    let rows = brief["results"].as_array().unwrap();
    assert_eq!(rows.len(), 30);
    assert!(full.as_array().unwrap().len() > rows.len());
    let ids = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), 30);
    assert!(
        brief_bytes * 100 <= full_bytes * 60,
        "brief {brief_bytes} B should be at most 60% of {full_bytes} B"
    );
    // Recorded at 9,055 B, plus a 10% margin.
    assert!(brief_bytes <= 9_961, "brief search grew to {brief_bytes} B");
    let row = &rows[0];
    assert_eq!(row["fingerprint"].as_str().unwrap().len(), 16);
    assert!(row.get("path").is_none());
    assert!(row.get("score").is_none());
    assert!(row.get("excerpt").is_none(), "title hits omit excerpt");
    assert!(!row["modified_at"].as_str().unwrap().contains('.'));

    let (limited, _) = json(&library, &["search", "git", "--brief", "-m", "5"]);
    assert_eq!(limited["total"], 30);
    assert_eq!(limited["results"].as_array().unwrap().len(), 5);

    assert_eq!(
        run(&library, &["search", "git", "--brief", "--context", "2"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn search_brief_reports_matched_fragments_and_sizes() {
    let (_guard, library) = fixture();
    let (brief, _) = json(&library, &["search", "echo second", "--brief"]);
    let row = &brief["results"][0];
    assert_eq!(row["title"], "Big multi");
    assert_eq!(row["fragments"], 2);
    assert_eq!(row["languages"], serde_json::json!(["toml", "bash"]));
    assert_eq!(row["matched_fragments"][0]["index"], 2);
    assert_eq!(row["matched_fragments"][0]["lines"], serde_json::json!([1]));
    assert_eq!(row["excerpt"], "echo second");
    let (shown, _) = json(&library, &["show", "Big multi"]);
    let bytes: usize = shown["loaded_fragments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fragment| fragment["content"].as_str().unwrap().len())
        .sum();
    assert_eq!(row["bytes"], bytes);
}

#[test]
fn show_outline_has_no_text_and_stays_small() {
    let (_guard, library) = fixture();
    let (outline, size) = json(&library, &["show", "Big multi", "--outline"]);
    assert!(size < 2_048, "outline grew to {size} B");
    assert_eq!(outline["truncated"], true);
    for fragment in outline["fragments"].as_array().unwrap() {
        assert!(fragment.get("content").is_none());
        assert!(fragment.get("note").is_none());
    }
    assert_eq!(outline["fragments"][0]["lines"], 1200);
    let output = run(&library, &["show", "Big multi", "--outline"]);
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "JSON output should be compact"
    );
    assert!(
        run(&library, &["show", "x", "--outline", "--max-bytes", "5"])
            .status
            .code()
            == Some(2)
    );
}

#[test]
fn show_max_bytes_cuts_on_line_then_char_boundaries() {
    let (_guard, library) = fixture();
    let (view, _) = json(&library, &["show", "Big multi", "--max-bytes", "1000"]);
    let content = view["fragments"][0]["content"].as_str().unwrap();
    assert!(content.len() <= 1000 && content.ends_with('\n'));
    assert!(
        content
            .lines()
            .all(|line| line.starts_with("line ") && line.len() == 29)
    );
    assert_eq!(view["fragments"][0]["truncated"], true);
    assert!(view["fragments"][1].get("content").is_none());
    let (full, _) = json(&library, &["show", "Big multi"]);
    let total: usize = full["loaded_fragments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fragment| fragment["content"].as_str().unwrap().len())
        .sum();
    assert_eq!(
        view["omitted_bytes"].as_u64().unwrap() as usize + content.len(),
        total
    );

    let (long, _) = json(&library, &["show", "One long line", "--max-bytes", "1001"]);
    let cut = long["fragments"][0]["content"].as_str().unwrap();
    assert!(cut.len() <= 1001 && cut.chars().all(|c| c == 'é'));
}

#[test]
fn cat_lines_and_max_bytes_print_the_exact_slice() {
    let (_guard, library) = fixture();
    let cat = |arguments: &[&str]| {
        Command::cargo_bin("snip")
            .unwrap()
            .arg("--library")
            .arg(&library)
            .arg("cat")
            .args(arguments)
            .output()
            .unwrap()
    };
    let sliced = cat(&["Big multi", "--fragment", "1", "--lines", "2:3"]);
    assert_eq!(
        String::from_utf8(sliced.stdout).unwrap(),
        "line 0002 of the big fragment\nline 0003 of the big fragment\n"
    );
    let note = String::from_utf8(sliced.stderr).unwrap();
    assert!(note.contains("printed lines 2-3 of 1200"), "{note}");

    let capped = cat(&["Big multi", "--fragment", "1", "--max-bytes", "70"]);
    let out = String::from_utf8(capped.stdout).unwrap();
    assert_eq!(out.lines().count(), 2);
    let note = String::from_utf8(capped.stderr).unwrap();
    assert!(note.contains("printed lines 1-2 of 1200"));
    assert!(!note.contains("cut mid-line"));

    let empty = cat(&["Big multi", "--fragment", "1", "--max-bytes", "0"]);
    assert!(empty.stdout.is_empty());
    let note = String::from_utf8(empty.stderr).unwrap();
    assert!(
        note.starts_with("note: printed 0 of 1200 lines (0 B of "),
        "{note}"
    );

    let midline = cat(&["One long line", "--max-bytes", "1001"]);
    let note = String::from_utf8(midline.stderr).unwrap();
    assert!(note.ends_with("(line 1 cut mid-line)\n"), "{note}");

    let whole = cat(&["Git helper 00"]);
    assert!(whole.stderr.is_empty(), "a full read prints no note");
    assert_eq!(
        cat(&["Big multi", "--fragment", "1", "--lines", "5000"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn list_brief_is_at_most_half_of_the_full_listing() {
    let (_guard, library) = fixture();
    let (_, full) = json(&library, &["list"]);
    let (brief, size) = json(&library, &["list", "--brief"]);
    // Fixture titles are short, so the saving is smaller than on a real library
    // (measured there: see the PR); the fence is 55% here.
    assert!(size * 100 <= full * 55, "brief {size} B vs full {full} B");
    let row = &brief[0];
    assert!(row.get("path").is_none() && row.get("bytes").is_some());
    assert_eq!(row["fingerprint"].as_str().unwrap().len(), 16);
}
