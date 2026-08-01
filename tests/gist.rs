#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use snip::Library;
use snip::service::{CreateOptions, FragmentAddOptions, add_fragment, create_snippet};
use tempfile::TempDir;

const STUB: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$GH_STUB_ARGV"
cat >> "$GH_STUB_STDIN"
if [ -n "$GH_STUB_STDERR" ]; then printf '%s\n' "$GH_STUB_STDERR" >&2; fi
if [ -n "$GH_STUB_EXIT" ] && [ "$GH_STUB_EXIT" != "0" ]; then exit "$GH_STUB_EXIT"; fi
if [ -n "$GH_STUB_RESPONSE" ]; then cat "$GH_STUB_RESPONSE"; fi
exit 0
"#;

const GIST_ID: &str = "5b0e0062eb8e9654adad7bb1d81cc75f";

fn library() -> (TempDir, Library) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Gist.sniplib");
    let library = Library::init(&root, Some("Gist")).unwrap();
    (temporary, library)
}

struct Stub {
    bin: PathBuf,
    argv: PathBuf,
    stdin: PathBuf,
    response: PathBuf,
    response_set: bool,
    stderr: Option<String>,
    exit: Option<String>,
}

impl Stub {
    fn new(temp: &Path) -> Self {
        fs::create_dir_all(temp).unwrap();
        let bin = temp.join("gh");
        fs::write(&bin, STUB).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            bin,
            argv: temp.join("argv.log"),
            stdin: temp.join("stdin.log"),
            response: temp.join("response.json"),
            response_set: false,
            stderr: None,
            exit: None,
        }
    }

    fn respond(&mut self, json: &str) -> &mut Self {
        fs::write(&self.response, json).unwrap();
        self.response_set = true;
        self
    }

    fn fail(&mut self, stderr: &str) -> &mut Self {
        self.stderr = Some(stderr.to_owned());
        self.exit = Some("1".to_owned());
        self
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(&self.argv)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn stdin_json(&self) -> serde_json::Value {
        let text = fs::read_to_string(&self.stdin).unwrap_or_default();
        if text.trim().is_empty() {
            return serde_json::Value::Null;
        }
        serde_json::from_str(&text).unwrap()
    }
}

fn snip(library: &Library, stub: &Stub, args: &[&str]) -> Command {
    let mut command = Command::cargo_bin("snip").unwrap();
    command
        .args(["--library", library.root().to_str().unwrap()])
        .env("SNIP_GH_BIN", &stub.bin)
        .env("GH_STUB_ARGV", &stub.argv)
        .env("GH_STUB_STDIN", &stub.stdin);
    if stub.response_set {
        command.env("GH_STUB_RESPONSE", &stub.response);
    }
    if let Some(stderr) = &stub.stderr {
        command.env("GH_STUB_STDERR", stderr);
    }
    if let Some(exit) = &stub.exit {
        command.env("GH_STUB_EXIT", exit);
    }
    command.args(args);
    command
}

fn create(library: &Library, title: &str) -> snip::Snippet {
    create_snippet(
        library,
        &CreateOptions {
            title: title.to_owned(),
            language: "text".to_owned(),
            content: format!("content of {title}\n"),
            readme: Some(format!("# {title}\n")),
            ..CreateOptions::default()
        },
    )
    .unwrap()
}

fn two_fragment_snippet(library: &Library, title: &str) -> snip::Snippet {
    let snippet = create_snippet(
        library,
        &CreateOptions {
            title: title.to_owned(),
            language: "text".to_owned(),
            content: format!("first of {title}\n"),
            readme: Some(format!("# {title}\n")),
            ..CreateOptions::default()
        },
    )
    .unwrap();
    add_fragment(
        library,
        &snippet.id.to_string(),
        &FragmentAddOptions {
            title: "Second".to_owned(),
            language: "text".to_owned(),
            content: "second\n".to_owned(),
            ..FragmentAddOptions::default()
        },
    )
    .unwrap()
    .0
}

fn gist_response(id: &str, public: bool, description: &str, files: &[&str]) -> String {
    let mut file_map = serde_json::Map::new();
    for name in files {
        file_map.insert(name.to_string(), serde_json::json!({ "filename": name }));
    }
    serde_json::to_string(&serde_json::json!({
        "id": id,
        "html_url": format!("https://gist.github.com/octocat/{id}"),
        "description": description,
        "public": public,
        "files": file_map,
    }))
    .unwrap()
}

fn snippet_toml(snippet: &snip::Snippet) -> toml::Value {
    let text = fs::read_to_string(snippet.package_path.join("snippet.toml")).unwrap();
    toml::from_str(&text).unwrap()
}

#[test]
fn push_creates_a_gist_with_expected_files() {
    let (temporary, library) = library();
    let snippet = two_fragment_snippet(&library, "Brewfile");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(
        GIST_ID,
        false,
        "Brewfile",
        &["001-Brewfile", "002-Second", "README.md"],
    ));

    snip(&library, &stub, &["gist", "push", "Brewfile"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "created gist: https://gist.github.com/octocat/{GIST_ID}"
        )))
        .stdout(predicate::str::contains("visibility: secret"))
        .stdout(predicate::str::contains(
            "files: 001-Brewfile, 002-Second, README.md",
        ));

    assert_eq!(stub.calls(), ["api --method POST /gists --input -"]);
    let body = stub.stdin_json();
    assert_eq!(body["description"], "Brewfile");
    assert_eq!(body["public"], false);
    let files = body["files"].as_object().unwrap();
    assert_eq!(files.len(), 3);
    assert!(files.contains_key("001-Brewfile"));
    assert!(files.contains_key("002-Second"));
    assert!(files.contains_key("README.md"));

    let manifest = snippet_toml(&snippet);
    let remotes = manifest["remotes"].as_array().unwrap();
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0]["kind"].as_str(), Some("gist"));
    assert_eq!(remotes[0]["id"].as_str(), Some(GIST_ID));
    assert_eq!(remotes[0]["host"].as_str(), Some("github.com"));
}

#[test]
fn push_updates_a_recorded_gist_and_deletes_gone_files() {
    let (temporary, library) = library();
    let snippet = create(&library, "Gone");
    fs::write(
        snippet.package_path.join("snippet.toml"),
        format!(
            "{}[[remotes]]\nkind = \"gist\"\nhost = \"github.com\"\nid = \"{GIST_ID}\"\nurl = \"https://gist.github.com/octocat/{GIST_ID}\"\npublic = false\ndescription = \"Gone\"\nfiles = [\"001-a.py\", \"002-gone.py\"]\npushed_at = \"2026-08-01T10:00:00Z\"\npushed_digest = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
            fs::read_to_string(snippet.package_path.join("snippet.toml")).unwrap(),
        ),
    )
    .unwrap();
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Gone", &["Gone"]));

    snip(&library, &stub, &["gist", "push", "Gone"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "updated gist: https://gist.github.com/octocat/{GIST_ID}"
        )));

    assert_eq!(
        stub.calls(),
        ["api --method PATCH /gists/5b0e0062eb8e9654adad7bb1d81cc75f --input -"]
    );
    let body = stub.stdin_json();
    let files = body["files"].as_object().unwrap();
    assert_eq!(files["002-gone.py"], serde_json::Value::Null);
    assert_eq!(files["001-a.py"], serde_json::Value::Null);
    assert_eq!(files["Gone"]["content"], "content of Gone\n");
}

#[test]
fn push_twice_is_idempotent_until_force() {
    let (temporary, library) = library();
    let snippet = create(&library, "Brewfile");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Brewfile", &["Brewfile"]));

    snip(&library, &stub, &["gist", "push", "Brewfile"])
        .assert()
        .success();
    assert_eq!(stub.calls().len(), 1);

    snip(&library, &stub, &["gist", "push", "Brewfile"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "gist is already up to date: https://gist.github.com/octocat/{GIST_ID}"
        )));
    assert_eq!(stub.calls().len(), 1, "unchanged push must not call gh");

    snip(&library, &stub, &["gist", "push", "Brewfile", "--force"])
        .assert()
        .success();
    assert_eq!(stub.calls().len(), 2, "--force pushes again");
    let _ = snippet;
}

#[test]
fn if_hash_mismatch_refuses_before_any_network_call() {
    let (temporary, library) = library();
    create(&library, "Brewfile");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Brewfile", &["Brewfile"]));

    snip(
        &library,
        &stub,
        &[
            "gist",
            "push",
            "Brewfile",
            "--if-hash",
            "deadbeefdeadbeefdeadbeefdeadbeef",
        ],
    )
    .assert()
    .code(4);
    assert!(
        stub.calls().is_empty(),
        "--if-hash mismatch must not call gh"
    );
}

#[test]
fn locked_snippet_can_be_pushed() {
    let (temporary, library) = library();
    create_snippet(
        &library,
        &CreateOptions {
            title: "Locked".to_owned(),
            language: "text".to_owned(),
            content: "locked\n".to_owned(),
            locked: true,
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Locked", &["Locked"]));

    snip(&library, &stub, &["gist", "push", "Locked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("created gist:"));
}

#[test]
fn public_on_a_recorded_secret_gist_is_rejected_without_a_call() {
    let (temporary, library) = library();
    let snippet = create(&library, "Brewfile");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Brewfile", &["Brewfile"]));

    snip(&library, &stub, &["gist", "push", "Brewfile"])
        .assert()
        .success();
    let fragment_path = snippet.loaded_fragments[0].absolute_path.clone();
    fs::write(&fragment_path, "changed\n").unwrap();

    snip(&library, &stub, &["gist", "push", "Brewfile", "--public"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains(
            "gist visibility cannot be changed after creation",
        ));
    assert_eq!(stub.calls().len(), 1, "rejected push must not call gh");
}

#[test]
fn empty_fragment_is_rejected_without_a_call() {
    let (temporary, library) = library();
    let snippet = create(&library, "Empty");
    let fragment_path = snippet.loaded_fragments[0].absolute_path.clone();
    fs::write(&fragment_path, "").unwrap();
    let stub = Stub::new(&temporary.path().join("stub"));

    snip(&library, &stub, &["gist", "push", "Empty"])
        .assert()
        .code(5)
        .stderr(predicate::str::contains(
            "cannot publish empty files to a gist",
        ));
    assert!(stub.calls().is_empty());
}

#[test]
fn missing_binary_reports_gh_not_found() {
    let (temporary, library) = library();
    create(&library, "Brewfile");
    let missing = temporary.path().join("does-not-exist/gh");
    snip(
        &library,
        &Stub::new(&temporary.path().join("stub")),
        &["gist", "push", "Brewfile"],
    )
    .env("SNIP_GH_BIN", missing)
    .assert()
    .code(1)
    .stderr(predicate::str::contains("gh not found in PATH"));
}

#[test]
fn http_404_on_update_means_the_gist_no_longer_exists() {
    let (temporary, library) = library();
    let snippet = create(&library, "Gone");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Gone", &["Gone"]));
    snip(&library, &stub, &["gist", "push", "Gone"])
        .assert()
        .success();

    let fragment_path = snippet.loaded_fragments[0].absolute_path.clone();
    fs::write(&fragment_path, "changed\n").unwrap();
    stub.fail("gh: Not Found (HTTP 404)");

    snip(&library, &stub, &["gist", "push", "Gone"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(format!(
            "gist {GIST_ID} no longer exists"
        )));
}

#[test]
fn url_on_an_unlinked_snippet_exits_3_in_human_and_json() {
    let (temporary, library) = library();
    create(&library, "Solo");
    let stub = Stub::new(&temporary.path().join("stub"));

    snip(&library, &stub, &["gist", "url", "Solo"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("snippet Solo has no gist"));
    snip(
        &library,
        &stub,
        &["--output", "json", "gist", "url", "Solo"],
    )
    .assert()
    .code(3)
    .stderr(predicate::str::contains("snippet Solo has no gist"));
}

#[test]
fn url_on_a_linked_snippet_prints_exactly_the_url() {
    let (temporary, library) = library();
    create(&library, "Solo");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Solo", &["Solo"]));
    snip(&library, &stub, &["gist", "push", "Solo"])
        .assert()
        .success();

    snip(&library, &stub, &["gist", "url", "Solo"])
        .assert()
        .success()
        .stdout(format!("https://gist.github.com/octocat/{GIST_ID}\n"));
}

#[test]
fn status_reports_clean_modified_and_unlinked_without_calling_gh() {
    let (temporary, library) = library();
    let snippet = create(&library, "Status");
    create(&library, "Fresh");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Status", &["Status"]));

    snip(&library, &stub, &["gist", "push", "Status"])
        .assert()
        .success();

    snip(&library, &stub, &["gist", "status", "Status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("state: clean"))
        .stdout(predicate::str::contains("pushed: "));
    assert_eq!(stub.calls().len(), 1, "status must not call gh");

    let fragment_path = snippet.loaded_fragments[0].absolute_path.clone();
    fs::write(&fragment_path, "changed\n").unwrap();

    snip(&library, &stub, &["gist", "status", "Status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("state: modified"));
    assert_eq!(stub.calls().len(), 1, "status must not call gh");

    snip(&library, &stub, &["gist", "status", "Fresh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("gist: none"));
    assert_eq!(stub.calls().len(), 1, "status must not call gh");
}

#[test]
fn status_requires_a_selector_or_all() {
    let (temporary, library) = library();
    create(&library, "Solo");
    let stub = Stub::new(&temporary.path().join("stub"));
    snip(&library, &stub, &["gist", "status"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("a selector or --all is required"));
}

#[test]
fn status_stays_clean_after_a_push_that_included_notes() {
    let (temporary, library) = library();
    create_snippet(
        &library,
        &CreateOptions {
            title: "Note".to_owned(),
            language: "text".to_owned(),
            content: "content\n".to_owned(),
            note: Some("a note\n".to_owned()),
            readme: Some("# Note\n".to_owned()),
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(
        GIST_ID,
        false,
        "Note",
        &["Note", "Note.note.md", "README.md"],
    ));

    snip(
        &library,
        &stub,
        &["gist", "push", "Note", "--include-notes"],
    )
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "files: Note, Note.note.md, README.md",
    ));

    snip(&library, &stub, &["gist", "status", "Note"])
        .assert()
        .success()
        .stdout(predicate::str::contains("state: clean"))
        .stdout(predicate::str::contains("pushed: "));
    assert_eq!(stub.calls().len(), 1, "status must not call gh");
}

#[test]
fn status_all_lists_only_linked_snippets() {
    let (temporary, library) = library();
    create(&library, "Linked");
    create(&library, "Fresh");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Linked", &["Linked"]));
    snip(&library, &stub, &["gist", "push", "Linked"])
        .assert()
        .success();

    snip(&library, &stub, &["gist", "status", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("snippet: Linked"))
        .stdout(predicate::str::contains("state: clean"))
        .stdout(predicate::str::contains("snippet: Fresh").not());
}

#[test]
fn status_remote_checks_the_remote_gist() {
    let (temporary, library) = library();
    create(&library, "Status");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Status", &["Status"]));
    snip(&library, &stub, &["gist", "push", "Status"])
        .assert()
        .success();

    snip(&library, &stub, &["gist", "status", "Status", "--remote"])
        .assert()
        .success()
        .stdout(predicate::str::contains("state: clean"));
    assert_eq!(
        stub.calls(),
        [
            "api --method POST /gists --input -",
            "api /gists/5b0e0062eb8e9654adad7bb1d81cc75f",
        ]
    );
}

#[test]
fn status_remote_reports_missing_when_the_gist_is_gone() {
    let (temporary, library) = library();
    create(&library, "Status");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Status", &["Status"]));
    snip(&library, &stub, &["gist", "push", "Status"])
        .assert()
        .success();

    stub.fail("gh: Not Found (HTTP 404)");
    snip(&library, &stub, &["gist", "status", "Status", "--remote"])
        .assert()
        .success()
        .stdout(predicate::str::contains("state: missing"));
    assert_eq!(stub.calls().len(), 2);
}

#[test]
fn status_remote_survives_an_error_echoing_http_in_the_description() {
    let (temporary, library) = library();
    create(&library, "Status");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Status", &["Status"]));
    snip(&library, &stub, &["gist", "push", "Status"])
        .assert()
        .success();

    stub.fail("gh: API call failed, the gist description says 'HTTP 404' but this is fine");
    snip(&library, &stub, &["gist", "status", "Status", "--remote"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("state: missing").not())
        .stderr(predicate::str::contains("gh failed:"));
}

#[test]
fn open_delegates_to_gh_gist_view_web() {
    let (temporary, library) = library();
    create(&library, "Status");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Status", &["Status"]));
    snip(&library, &stub, &["gist", "push", "Status"])
        .assert()
        .success();

    snip(&library, &stub, &["gist", "open", "Status"])
        .assert()
        .success();
    assert_eq!(
        stub.calls(),
        [
            "api --method POST /gists --input -",
            "gist view 5b0e0062eb8e9654adad7bb1d81cc75f --web",
        ]
    );
}

#[test]
fn attach_records_fetched_metadata_and_detach_removes_it() {
    let (temporary, library) = library();
    let snippet = create(&library, "Adopted");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, true, "Adopted", &["Adopted"]));

    snip(&library, &stub, &["gist", "attach", "Adopted", GIST_ID])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "attached gist: https://gist.github.com/octocat/{GIST_ID}"
        )));
    assert_eq!(
        stub.calls(),
        ["api /gists/5b0e0062eb8e9654adad7bb1d81cc75f"]
    );
    let manifest = snippet_toml(&snippet);
    assert_eq!(manifest["remotes"][0]["id"].as_str(), Some(GIST_ID));
    assert_eq!(manifest["remotes"][0]["public"].as_bool(), Some(true));

    snip(&library, &stub, &["gist", "attach", "Adopted", GIST_ID])
        .assert()
        .code(4)
        .stderr(predicate::str::contains("is already linked to gist"));

    snip(&library, &stub, &["gist", "detach", "Adopted"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "detached gist: {GIST_ID}"
        )));

    let text = fs::read_to_string(snippet.package_path.join("snippet.toml")).unwrap();
    assert!(!text.contains("remotes"), "detach leaves no remotes key");
}

#[test]
fn delete_removes_the_gist_and_the_record() {
    let (temporary, library) = library();
    let snippet = create(&library, "Doomed");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Doomed", &["Doomed"]));
    snip(&library, &stub, &["gist", "push", "Doomed"])
        .assert()
        .success();
    stub.response_set = false;

    snip(&library, &stub, &["gist", "delete", "Doomed", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("deleted gist: {GIST_ID}")));
    assert_eq!(
        stub.calls().last().map(String::as_str),
        Some("api --method DELETE /gists/5b0e0062eb8e9654adad7bb1d81cc75f")
    );
    let text = fs::read_to_string(snippet.package_path.join("snippet.toml")).unwrap();
    assert!(!text.contains("remotes"), "delete leaves no remotes key");
}

#[test]
fn delete_without_yes_under_json_is_a_usage_error() {
    let (temporary, library) = library();
    create(&library, "Doomed");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Doomed", &["Doomed"]));
    snip(&library, &stub, &["gist", "push", "Doomed"])
        .assert()
        .success();

    snip(
        &library,
        &stub,
        &["--output", "json", "gist", "delete", "Doomed"],
    )
    .assert()
    .code(2)
    .stderr(predicate::str::contains(
        "--yes is required when output is not human-readable",
    ));
    assert_eq!(stub.calls().len(), 1, "refused delete must not call gh");
}

#[test]
fn push_json_contract_contains_every_expected_key() {
    let (temporary, library) = library();
    create(&library, "Brewfile");
    let mut stub = Stub::new(&temporary.path().join("stub"));
    stub.respond(&gist_response(GIST_ID, false, "Brewfile", &["Brewfile"]));

    let output = snip(
        &library,
        &stub,
        &["--output", "json", "gist", "push", "Brewfile"],
    )
    .output()
    .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["action"], "created");
    assert!(value["snippet"]["id"].is_string());
    assert_eq!(value["snippet"]["title"], "Brewfile");
    assert!(value["snippet"]["folder"].is_string());
    assert!(value["fingerprint"].is_string());
    let gist = &value["gist"];
    assert_eq!(gist["kind"], "gist");
    assert_eq!(gist["host"], "github.com");
    assert_eq!(gist["id"], GIST_ID);
    assert_eq!(
        gist["url"],
        format!("https://gist.github.com/octocat/{GIST_ID}")
    );
    assert_eq!(gist["public"], false);
    assert_eq!(gist["description"], "Brewfile");
    assert_eq!(gist["files"], serde_json::json!(["Brewfile", "README.md"]));
    assert!(gist["pushed_at"].is_string());
    assert!(gist["pushed_digest"].is_string());
}
