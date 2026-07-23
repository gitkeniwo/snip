use snip::render::{RenderMode, preview};
use snip::search::{MemoryIndex, SearchIndex};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, add_fragment, create_snippet, delete_snippet,
    doctor, edit_snippet, restore_snippet, trash_entries,
};
use snip::{
    AppConfig, ErrorKind, Fingerprint, Library, OutputSetting, TuiIconSetting, TuiSortSetting,
    TuiThemeSetting,
};
use std::fs;
use tempfile::TempDir;

fn library() -> (TempDir, Library) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Test.sniplib");
    let library = Library::init(&root, Some("Test")).unwrap();
    (temporary, library)
}

fn create_example(library: &Library, locked: bool) -> snip::Snippet {
    create_snippet(
        library,
        &CreateOptions {
            title: "你好 Script".to_owned(),
            folder: Some("Examples/Shell".to_owned()),
            tags: vec![" demo ".to_owned(), "DEMO".to_owned()],
            language: "bash".to_owned(),
            fragment_title: Some("Main".to_owned()),
            content: "echo hello\n".to_owned(),
            note: Some("**Greeting** note".to_owned()),
            locked,
            ..CreateOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn filesystem_is_the_source_of_truth() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    assert_eq!(snippet.tags, vec!["demo"]);
    assert_eq!(
        library.scan().unwrap().folders,
        vec!["Examples", "Examples/Shell"]
    );
    assert_eq!(library.scan().unwrap().tags, vec!["demo"]);

    let old_hash = snippet.fingerprint.clone();
    fs::write(&snippet.loaded_fragments[0].absolute_path, "echo changed\n").unwrap();
    let catalog = library.scan().unwrap();
    let changed = library
        .resolve_snippet(&catalog, &snippet.id.to_string())
        .unwrap();
    assert_ne!(old_hash, changed.fingerprint);
    assert_eq!(changed.loaded_fragments[0].content, "echo changed\n");

    let results = MemoryIndex::new(catalog).search("changed", None, None);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line, Some(1));
}

#[test]
fn optimistic_hash_and_lock_are_enforced() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, true);
    let error = edit_snippet(
        &library,
        &snippet.id.to_string(),
        &EditOptions {
            content: Some("new".to_owned()),
            if_hash: Some(Fingerprint("wrong".to_owned())),
            ..EditOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);

    let error = edit_snippet(
        &library,
        &snippet.id.to_string(),
        &EditOptions {
            content: Some("new".to_owned()),
            if_hash: Some(snippet.fingerprint.clone()),
            ..EditOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);

    let (updated, _) = edit_snippet(
        &library,
        &snippet.id.to_string(),
        &EditOptions {
            content: Some("new".to_owned()),
            if_hash: Some(snippet.fingerprint),
            force: true,
            ..EditOptions::default()
        },
    )
    .unwrap();
    assert_eq!(updated.loaded_fragments[0].content, "new");
}

#[test]
fn fragment_and_trash_lifecycle_round_trip() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let (snippet, _) = add_fragment(
        &library,
        &snippet.id.to_string(),
        &FragmentAddOptions {
            title: "Second".to_owned(),
            language: "markdown".to_owned(),
            content: "# Two\n".to_owned(),
            if_hash: Some(snippet.fingerprint),
            ..FragmentAddOptions::default()
        },
    )
    .unwrap();
    assert_eq!(snippet.loaded_fragments.len(), 2);

    let entry = delete_snippet(
        &library,
        &snippet.id.to_string(),
        Some(&snippet.fingerprint),
        false,
    )
    .unwrap();
    assert!(library.scan().unwrap().snippets.is_empty());
    assert_eq!(trash_entries(&library).unwrap().len(), 1);

    let restored = restore_snippet(&library, &entry.entry_id, None).unwrap();
    assert_eq!(restored.id, snippet.id);
    assert_eq!(restored.loaded_fragments.len(), 2);
    assert!(doctor(&library, false).ok);
}

#[test]
fn unknown_manifest_fields_survive_cli_edit() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let path = snippet.package_path.join("snippet.toml");
    let text = fs::read_to_string(&path).unwrap().replace(
        "[[fragments]]",
        "custom_gui_hint = \"wide\"\n\n[[fragments]]",
    );
    fs::write(&path, text).unwrap();
    let fresh = library.scan().unwrap().snippets.remove(0);
    edit_snippet(
        &library,
        &fresh.id.to_string(),
        &EditOptions {
            title: Some("Renamed".to_owned()),
            if_hash: Some(fresh.fingerprint),
            ..EditOptions::default()
        },
    )
    .unwrap();
    let edited = library.scan().unwrap().snippets.remove(0);
    assert_eq!(
        edited
            .extra
            .get("custom_gui_hint")
            .and_then(toml::Value::as_str),
        Some("wide")
    );
}

#[test]
fn traversal_and_symlinks_are_rejected() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let manifest_path = snippet.package_path.join("snippet.toml");
    let text = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("fragments/001-你好 Script.sh", "../outside");
    fs::write(&manifest_path, text).unwrap();
    assert_eq!(library.scan().unwrap_err().kind, ErrorKind::Validation);
}

#[cfg(unix)]
#[test]
fn managed_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let content_path = &snippet.loaded_fragments[0].absolute_path;
    fs::remove_file(content_path).unwrap();
    symlink("/etc/hosts", content_path).unwrap();
    assert_eq!(library.scan().unwrap_err().kind, ErrorKind::Validation);
}

#[test]
fn preview_supports_plain_ansi_and_html() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let plain = preview(&snippet, RenderMode::Plain, false).unwrap();
    let ansi = preview(&snippet, RenderMode::Ansi, true).unwrap();
    let html = preview(&snippet, RenderMode::Html, true).unwrap();
    assert!(plain.contains("echo hello"));
    assert!(ansi.contains("\u{1b}["));
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("echo"));
}

#[test]
fn doctor_recovers_an_interrupted_package_swap() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let transaction = library.transactions_dir().join("test-transaction");
    fs::create_dir_all(&transaction).unwrap();
    let original = snippet
        .package_path
        .strip_prefix(library.root())
        .unwrap()
        .to_string_lossy()
        .to_string();
    fs::write(
        transaction.join("transaction.toml"),
        format!(
            "schema_version = 1\noperation = \"replace\"\noriginal_path = {original:?}\ntarget_path = {original:?}\n"
        ),
    )
    .unwrap();
    fs::rename(&snippet.package_path, transaction.join("backup")).unwrap();

    let before = doctor(&library, false);
    assert!(!before.ok);
    assert_eq!(before.pending_transactions.len(), 1);
    let repaired = doctor(&library, true);
    assert!(repaired.ok);
    assert_eq!(library.scan().unwrap().snippets.len(), 1);
}

#[test]
fn config_round_trip_preserves_unknown_fields_and_normalizes_tags() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.toml");
    fs::write(
        &path,
        r##"schema_version = 1
output = "json"
default_tags = [" demo ", "DEMO", "Rust"]
future_gui_layout = "wide"

[tui]
theme = "light"
sort = "modified"
icons = "nerd"

[tui.colors]
accent = "#123456"
"##,
    )
    .unwrap();

    let config = AppConfig::load_from(&path).unwrap();
    assert_eq!(config.output, Some(OutputSetting::Json));
    assert_eq!(config.default_tags, vec!["demo", "Rust"]);
    let tui = config.tui.as_ref().unwrap();
    assert_eq!(tui.theme, TuiThemeSetting::Light);
    assert_eq!(tui.sort, TuiSortSetting::Modified);
    assert_eq!(tui.icons, TuiIconSetting::Nerd);
    assert_eq!(
        tui.extra
            .get("colors")
            .and_then(|value| value.as_table())
            .and_then(|colors| colors.get("accent"))
            .and_then(|value| value.as_str()),
        Some("#123456")
    );
    assert_eq!(
        config
            .extra
            .get("future_gui_layout")
            .and_then(|v| v.as_str()),
        Some("wide")
    );

    config.save_to(&path).unwrap();
    let saved = fs::read_to_string(path).unwrap();
    assert!(saved.contains("future_gui_layout = \"wide\""));
    assert!(saved.contains("accent = \"#123456\""));
}
