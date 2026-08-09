use snip::render::{RenderMode, preview};
use snip::search::{MemoryIndex, SearchIndex, SearchQuery};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, add_fragment, create_snippet, delete_snippet,
    doctor, edit_snippet, restore_snippet, trash_entries,
};
use snip::{
    AppConfig, ErrorKind, Fingerprint, Library, OutputSetting, SortMode, TuiDensitySetting,
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
fn open_restores_recreatable_directories_and_remains_writable() {
    for missing in [".snip", "snippets", "trash"] {
        let (_temporary, library) = library();
        let root = library.root().to_path_buf();
        fs::remove_dir_all(root.join(missing)).unwrap();

        let restored = Library::open(&root).unwrap();
        assert!(root.join("snippets").is_dir());
        assert!(root.join("trash").is_dir());
        assert!(root.join(".snip/cache").is_dir());
        assert!(root.join(".snip/locks").is_dir());
        assert!(root.join(".snip/transactions").is_dir());
        create_example(&restored, false);
    }
}

#[test]
fn open_reports_every_restored_directory_in_order() {
    let (_temporary, library) = library();
    let root = library.root().to_path_buf();
    fs::remove_dir_all(root.join("snippets")).unwrap();
    fs::remove_dir_all(root.join("trash")).unwrap();
    fs::remove_dir_all(root.join(".snip")).unwrap();

    let restored = Library::open(&root).unwrap();
    assert_eq!(
        restored.restored(),
        [
            "snippets",
            "trash",
            ".snip/cache",
            ".snip/locks",
            ".snip/transactions",
        ]
    );
}

#[test]
fn complete_library_has_no_restored_directories() {
    let (_temporary, library) = library();
    assert!(Library::open(library.root()).unwrap().restored().is_empty());
}

#[test]
fn display_name_strips_a_trailing_sniplib_suffix() {
    let temporary = tempfile::tempdir().unwrap();
    // `--name Main.sniplib` lands verbatim in snip.toml; display strips it.
    let library =
        Library::init(&temporary.path().join("Main.sniplib"), Some("Main.sniplib")).unwrap();
    assert_eq!(library.display_name(), "Main");

    // A plain name is shown whole.
    let named = Library::init(&temporary.path().join("Archive.sniplib"), Some("Archive")).unwrap();
    assert_eq!(named.display_name(), "Archive");

    // The inferred default comes from the directory stem and has no suffix.
    let inferred = Library::init(&temporary.path().join("Inferred.sniplib"), None).unwrap();
    assert_eq!(inferred.display_name(), "Inferred");
}

#[test]
fn invalid_manifest_does_not_create_library_directories() {
    for manifest in ["", "not valid toml = ["] {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(temporary.path().join("snip.toml"), manifest).unwrap();

        assert!(Library::open(temporary.path()).is_err());
        assert!(!temporary.path().join("snippets").exists());
        assert!(!temporary.path().join("trash").exists());
        assert!(!temporary.path().join(".snip").exists());
    }
}

#[test]
fn doctor_reports_directories_restored_during_open() {
    let (_temporary, library) = library();
    let root = library.root().to_path_buf();
    fs::remove_dir_all(root.join(".snip/locks")).unwrap();

    let restored = Library::open(&root).unwrap();
    let report = doctor(&restored, false);
    assert!(report.ok);
    assert!(
        report
            .repaired
            .contains(&"recreated missing directory: .snip/locks".to_owned())
    );
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

    let results = MemoryIndex::new(catalog).search(&SearchQuery::new("changed", false).unwrap());
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
sort = "manual"
density = "compact"

[tui.colors]
accent = "#123456"

[git]
auto_commit_interval = 15
auto_push = true
auto_pull = true
backup_on_quit = true
future_remote_policy = "manual"
"##,
    )
    .unwrap();

    let config = AppConfig::load_from(&path).unwrap();
    assert_eq!(config.output, Some(OutputSetting::Json));
    assert_eq!(config.default_tags, vec!["demo", "Rust"]);
    let tui = config.tui.as_ref().unwrap();
    assert_eq!(tui.theme, TuiThemeSetting::Light);
    assert_eq!(tui.sort, SortMode::Modified);
    assert_eq!(tui.density, TuiDensitySetting::Compact);
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
    assert_eq!(
        config.git.as_ref().map(|git| (
            git.auto_commit_interval,
            git.auto_push,
            git.auto_pull,
            git.backup_on_quit,
            git.extra
                .get("future_remote_policy")
                .and_then(toml::Value::as_str)
        )),
        Some((15, true, true, true, Some("manual")))
    );

    config.save_to(&path).unwrap();
    let saved = fs::read_to_string(path).unwrap();
    assert!(saved.contains("future_gui_layout = \"wide\""));
    assert!(saved.contains("accent = \"#123456\""));
    assert!(saved.contains("sort = \"modified\""));
    assert!(saved.contains("future_remote_policy = \"manual\""));
}

#[test]
fn legacy_auto_backup_interval_loads_and_is_rewritten_with_the_precise_name() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.toml");
    fs::write(
        &path,
        "schema_version = 1\n\n[git]\nauto_backup_interval = 15\n",
    )
    .unwrap();

    let config = AppConfig::load_from(&path).unwrap();
    assert_eq!(
        config.git.as_ref().map(|git| git.auto_commit_interval),
        Some(15)
    );

    config.save_to(&path).unwrap();
    let saved = fs::read_to_string(path).unwrap();
    assert!(saved.contains("auto_commit_interval = 15"));
    assert!(!saved.contains("auto_backup_interval"));
}
