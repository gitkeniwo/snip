use snip::render::{RenderMode, preview};
use snip::search::{MemoryIndex, SearchIndex, SearchQuery};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, add_fragment, create_snippet, delete_snippet,
    doctor, edit_snippet, restore_snippet, trash_entries,
};
use snip::{
    AppConfig, EditorCwdSetting, ErrorKind, Fingerprint, Library, OutputSetting, SortMode,
    TuiDensitySetting, TuiThemeSetting,
};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

fn library() -> (TempDir, Library) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Test.sniplib");
    let library = Library::init(&root, Some("Test")).unwrap();
    (temporary, library)
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
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
fn conformance_fixture_has_stable_boundaries() {
    let temporary = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance.sniplib");
    let root = temporary.path().join("Conformance.sniplib");
    copy_tree(&source, &root);

    assert!(!root.join("tags.toml").exists());
    let library = Library::open(&root).unwrap();
    let catalog = library.scan().unwrap();
    assert_eq!(catalog.snippets.len(), 1);
    assert_eq!(catalog.snippets[0].title, "互操作性 🧪");
    assert_eq!(catalog.snippets[0].loaded_fragments[0].content, "");
    assert_eq!(
        catalog.snippets[0].fingerprint.0,
        "633c2c21f9a0bf2e42fd65982ddf95486f91b764e746f6d9577ca8d6f1d3e089"
    );
    assert_eq!(
        catalog.library.extra["fixture_extension"].as_str(),
        Some("preserve me")
    );
    assert_eq!(
        catalog.snippets[0].extra["fixture_extension"].as_str(),
        Some("preserve me too")
    );
    assert_eq!(trash_entries(&library).unwrap().len(), 1);
    assert!(doctor(&library, false).ok);
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
fn init_ignores_the_entire_runtime_tree() {
    let (_temporary, library) = library();
    let ignore = fs::read_to_string(library.root().join(".gitignore")).unwrap();
    assert!(ignore.lines().any(|line| line == ".snip/"));
    assert!(!ignore.lines().any(|line| line == ".snip/cache/"));
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
fn open_keeps_legacy_library_metadata_available_to_doctor() {
    let (_temporary, library) = library();
    let root = library.root().to_path_buf();
    let manifest_path = root.join("snip.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let manifest = manifest
        .replace("name = \"Test\"", "name = \"\"")
        .replace(&library.manifest().created_at, "2026-08-13 12:00:00");
    fs::write(&manifest_path, manifest).unwrap();

    let opened = Library::open(&root).unwrap();
    assert!(opened.scan().is_ok());
    let report = doctor(&opened, false);
    assert!(!report.ok);
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("empty library name"))
    );
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains("invalid RFC 3339"))
    );
}

#[test]
fn init_rejects_an_empty_name_before_creating_the_library() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Empty.sniplib");
    assert_eq!(
        Library::init(&root, Some(" ")).unwrap_err().kind,
        ErrorKind::Usage
    );
    assert!(!root.exists());
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

    let results =
        MemoryIndex::new(Arc::new(catalog)).search(&SearchQuery::new("changed", false).unwrap());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line, Some(1));
}

#[test]
fn new_snippet_folders_use_the_portable_path_grammar() {
    let (_temporary, library) = library();
    for folder in ["Bad:Folder", "Bad\\Folder", "Bad//Folder", "Bad/../Folder"] {
        let error = create_snippet(
            &library,
            &CreateOptions {
                title: "Portable".to_owned(),
                folder: Some(folder.to_owned()),
                language: "text".to_owned(),
                ..CreateOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Validation, "{folder:?}");
    }
}

#[cfg(unix)]
#[test]
fn legacy_nonportable_folder_can_be_read_edited_and_moved() {
    use snip::service::move_folder;

    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let second = create_example(&library, false);
    let legacy_folder = library.snippets_dir().join("Legacy:Notes");
    fs::create_dir_all(&legacy_folder).unwrap();
    let legacy_package = legacy_folder.join(snippet.package_path.file_name().unwrap());
    fs::rename(&snippet.package_path, &legacy_package).unwrap();
    let second_legacy_package = legacy_folder.join(second.package_path.file_name().unwrap());
    fs::rename(&second.package_path, &second_legacy_package).unwrap();

    let catalog = library.scan().unwrap();
    let legacy = catalog
        .snippets
        .iter()
        .find(|candidate| candidate.id == snippet.id)
        .unwrap();
    assert_eq!(legacy.folder, "Legacy:Notes");
    let report = doctor(&library, false);
    assert!(report.ok);
    assert_eq!(report.checked, 2);
    assert_eq!(report.warnings.len(), 1);

    let (edited, _) = edit_snippet(
        &library,
        &legacy.id.to_string(),
        &EditOptions {
            content: Some("still editable".to_owned()),
            if_hash: Some(legacy.fingerprint.clone()),
            ..EditOptions::default()
        },
    )
    .unwrap();
    assert_eq!(edited.loaded_fragments[0].content, "still editable");
    move_folder(&library, "Legacy:Notes", "Portable/Notes").unwrap();
    assert!(
        library
            .scan()
            .unwrap()
            .snippets
            .iter()
            .all(|snippet| snippet.folder == "Portable/Notes")
    );
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

#[cfg(unix)]
#[test]
fn unreferenced_package_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    symlink(
        "/etc/hosts",
        snippet.package_path.join("attachments/host-link"),
    )
    .unwrap();
    assert_eq!(library.scan().unwrap_err().kind, ErrorKind::Validation);
}

#[test]
fn legacy_metadata_errors_remain_readable_and_are_reported_by_doctor() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let manifest_path = snippet.package_path.join("snippet.toml");
    let original = fs::read_to_string(&manifest_path).unwrap();

    fs::write(
        &manifest_path,
        original.replace(&snippet.created_at, "not-a-timestamp"),
    )
    .unwrap();
    assert_eq!(library.scan().unwrap().snippets.len(), 1);
    assert!(!doctor(&library, false).ok);

    let remote = r#"
[[remotes]]
kind = "gist"
host = "github.com"
id = "one"
url = "https://gist.github.com/one"

[[remotes]]
kind = "gist"
host = "github.com"
id = "two"
url = "https://gist.github.com/two"
"#;
    fs::write(&manifest_path, format!("{original}{remote}")).unwrap();
    assert_eq!(library.scan().unwrap().snippets.len(), 1);
    assert!(!doctor(&library, false).ok);
}

#[test]
fn writers_reject_invalid_semantic_metadata() {
    let (_temporary, library) = library();
    let error = create_snippet(
        &library,
        &CreateOptions {
            title: "Invalid timestamp".to_owned(),
            language: "text".to_owned(),
            created_at: Some("not-a-timestamp".to_owned()),
            ..CreateOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Validation);
}

#[test]
fn blocked_replacement_cleans_its_transaction_and_reports_the_source_manifest() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let manifest_path = snippet.package_path.join("snippet.toml");
    let original = fs::read_to_string(&manifest_path).unwrap();
    fs::write(
        &manifest_path,
        original.replace(&snippet.created_at, "2026-03-15 10:20:00"),
    )
    .unwrap();
    let current = library.scan().unwrap().snippets.remove(0);

    let error = edit_snippet(
        &library,
        &current.id.to_string(),
        &EditOptions {
            title: Some("Renamed".to_owned()),
            if_hash: Some(current.fingerprint),
            ..EditOptions::default()
        },
    )
    .unwrap_err();

    assert_eq!(error.kind, ErrorKind::Validation);
    assert!(error.message.contains(&manifest_path.display().to_string()));
    assert!(!error.message.contains("transactions"));
    assert_eq!(
        error.hint.as_deref(),
        Some("run snip doctor to list metadata that blocks writes, then fix it by hand")
    );
    assert!(
        fs::read_dir(library.transactions_dir())
            .unwrap()
            .next()
            .is_none()
    );
    assert!(doctor(&library, false).pending_transactions.is_empty());
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
        .replace('\\', "/");
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
fn doctor_removes_an_incomplete_transaction_without_a_backup() {
    let (_temporary, library) = library();
    let transaction = library.transactions_dir().join("incomplete-stage");
    fs::create_dir_all(transaction.join("staged")).unwrap();
    fs::write(transaction.join("staged/snippet.toml"), "incomplete").unwrap();

    let repaired = doctor(&library, true);
    assert!(repaired.ok, "{:?}", repaired.errors);
    assert!(!transaction.exists());
    assert!(
        repaired
            .repaired
            .iter()
            .any(|message| message == "removed incomplete transaction incomplete-stage")
    );
}

#[test]
fn doctor_preserves_a_backup_when_transaction_state_is_missing() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let transaction = library.transactions_dir().join("missing-state-with-backup");
    fs::create_dir_all(&transaction).unwrap();
    let backup = transaction.join("backup");
    fs::rename(&snippet.package_path, &backup).unwrap();

    let report = doctor(&library, true);
    assert!(!report.ok);
    assert!(backup.exists());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains(&backup.display().to_string()))
    );
}

#[test]
fn doctor_restores_a_valid_backup_over_an_invalid_transaction_target() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let transaction = library.transactions_dir().join("invalid-target");
    fs::create_dir_all(&transaction).unwrap();
    let original = snippet
        .package_path
        .strip_prefix(library.root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        transaction.join("transaction.toml"),
        format!(
            "schema_version = 1\noperation = \"replace\"\noriginal_path = {original:?}\ntarget_path = {original:?}\n"
        ),
    )
    .unwrap();
    fs::rename(&snippet.package_path, transaction.join("backup")).unwrap();
    fs::create_dir_all(&snippet.package_path).unwrap();
    fs::write(snippet.package_path.join("snippet.toml"), "invalid = [").unwrap();

    let repaired = doctor(&library, true);
    assert!(repaired.ok, "{:?}", repaired.errors);
    assert_eq!(library.scan().unwrap().snippets.len(), 1);
}

#[test]
fn doctor_keeps_an_invalid_backup_and_reports_its_rescue_path() {
    let (_temporary, library) = library();
    let snippet = create_example(&library, false);
    let transaction = library.transactions_dir().join("invalid-backup");
    fs::create_dir_all(&transaction).unwrap();
    let original = snippet
        .package_path
        .strip_prefix(library.root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        transaction.join("transaction.toml"),
        format!(
            "schema_version = 1\noperation = \"replace\"\noriginal_path = {original:?}\ntarget_path = {original:?}\n"
        ),
    )
    .unwrap();
    let backup = transaction.join("backup");
    fs::rename(&snippet.package_path, &backup).unwrap();
    fs::write(backup.join("snippet.toml"), "invalid = [").unwrap();

    let report = doctor(&library, true);
    assert!(!report.ok);
    assert!(transaction.exists());
    assert!(
        report
            .errors
            .iter()
            .any(|error| error.contains(&backup.display().to_string()))
    );
}

#[test]
fn trash_and_transaction_paths_cannot_escape_the_library() {
    let (temporary, library) = library();
    let snippet = create_example(&library, false);
    let entry = delete_snippet(
        &library,
        &snippet.id.to_string(),
        Some(&snippet.fingerprint),
        false,
    )
    .unwrap();
    let metadata_path = entry.package_path.parent().unwrap().join("trash.toml");
    let metadata = fs::read_to_string(&metadata_path)
        .unwrap()
        .replace(&entry.original_path, "../escaped");
    fs::write(&metadata_path, metadata).unwrap();
    assert_eq!(
        trash_entries(&library).unwrap_err().kind,
        ErrorKind::Validation
    );
    assert!(!temporary.path().join("escaped").exists());

    let transaction = library.transactions_dir().join("escaping-transaction");
    fs::create_dir_all(&transaction).unwrap();
    fs::write(
        transaction.join("transaction.toml"),
        "schema_version = 1\noperation = \"replace\"\noriginal_path = \"../outside\"\ntarget_path = \"../outside\"\n",
    )
    .unwrap();
    let report = doctor(&library, true);
    assert!(!report.ok);
    assert!(!report.errors.is_empty());
    assert!(
        transaction.exists(),
        "failed recovery must remain inspectable"
    );
}

#[test]
fn config_round_trip_preserves_unknown_fields_and_normalizes_tags() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("config.toml");
    fs::write(
        &path,
        r##"schema_version = 1
output = "json"
editor_cwd = "snippet"
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
    assert_eq!(config.editor_cwd, Some(EditorCwdSetting::Snippet));
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
