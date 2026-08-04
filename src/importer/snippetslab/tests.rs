use plist::{Dictionary, Uid, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::mapping::{build_folder_paths, key, map_language};
use super::types::LegacyFolder;
use super::*;
use crate::filesystem::Library;
use crate::service::doctor;

fn dict(entries: Vec<(String, Value)>) -> Value {
    Value::Dictionary(entries.into_iter().collect())
}

fn archive(root: Value) -> Vec<u8> {
    let mut top = Dictionary::new();
    top.insert("root".to_owned(), Value::Uid(Uid::new(1)));
    let mut archive = Dictionary::new();
    archive.insert(
        "$archiver".to_owned(),
        Value::String("NSKeyedArchiver".to_owned()),
    );
    archive.insert(
        "$objects".to_owned(),
        Value::Array(vec![Value::String("$null".to_owned()), root]),
    );
    archive.insert("$top".to_owned(), Value::Dictionary(top));
    let mut output = Vec::new();
    Value::Dictionary(archive)
        .to_writer_binary(&mut output)
        .unwrap();
    output
}

fn write_archive(path: &Path, root: Value) {
    fs::write(path, archive(root)).unwrap();
}

const FIXTURE_DIR: &str = "src/importer/snippetslab/fixtures/snippetslab-2.6.snippetslablibrary";

fn committed_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR)
}

fn copy_dir_recursive(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        let file_type = entry.file_type().unwrap();
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn legacy_fixture(root: &Path) -> (Uuid, Uuid, Uuid) {
    let snippet_id = Uuid::new_v4();
    let first_fragment_id = Uuid::new_v4();
    let second_fragment_id = Uuid::new_v4();
    let tag_id = Uuid::new_v4();
    let parent_id = Uuid::new_v4();
    let child_id = Uuid::new_v4();
    fs::create_dir_all(root.join("Database/Snippets")).unwrap();
    fs::create_dir_all(root.join("Database/Attachments")).unwrap();
    fs::write(root.join("Database/Attachments/legacy.bin"), b"attachment").unwrap();

    write_archive(
        &root.join("identifier"),
        Value::String("legacy-library-id".to_owned()),
    );
    write_archive(
        &root.join("version.plist"),
        dict(vec![(
            "SnippetsLab".to_owned(),
            Value::String("2.6".to_owned()),
        )]),
    );

    let child = dict(vec![
        (key("NodeUUID"), Value::String(child_id.to_string())),
        (key("NodeTitle"), Value::String("Child".to_owned())),
    ]);
    let parent = dict(vec![
        (key("NodeUUID"), Value::String(parent_id.to_string())),
        (key("NodeTitle"), Value::String("Parent".to_owned())),
        (key("NodeChildren"), Value::Array(vec![child])),
    ]);
    write_archive(
        &root.join("Database/folders.data"),
        Value::Array(vec![Value::Data(archive(parent))]),
    );

    let tag = dict(vec![
        (key("TagUUID"), Value::String(tag_id.to_string())),
        (key("TagTitle"), Value::String(" dev ".to_owned())),
        (key("TagColor"), Value::Integer(3.into())),
    ]);
    write_archive(
        &root.join("Database/tags.data"),
        Value::Array(vec![Value::Data(archive(tag))]),
    );

    let first = dict(vec![
        (
            key("SnippetPartUUID"),
            Value::String(first_fragment_id.to_string()),
        ),
        (key("SnippetPartTitle"), Value::String("run.sh".to_owned())),
        (
            key("SnippetPartLanguage"),
            Value::String("BashLexer".to_owned()),
        ),
        (
            key("SnippetPartContent"),
            Value::Data(b"echo imported\n".to_vec()),
        ),
        (key("SnippetPartNote"), Value::Data(b"first note".to_vec())),
    ]);
    let second = dict(vec![
        (
            key("SnippetPartUUID"),
            Value::String(second_fragment_id.to_string()),
        ),
        (key("SnippetPartTitle"), Value::String("readme".to_owned())),
        (
            key("SnippetPartLanguage"),
            Value::String("MarkdownLexer".to_owned()),
        ),
        (
            key("SnippetPartContent"),
            Value::Data(b"# Imported\n".to_vec()),
        ),
        (key("SnippetPartNote"), Value::Data(Vec::new())),
    ]);
    let snippet = dict(vec![
        (key("SnippetUUID"), Value::String(snippet_id.to_string())),
        (
            key("SnippetTitle"),
            Value::String("Imported snippet".to_owned()),
        ),
        (
            key("SnippetFolderUUID"),
            Value::String(child_id.to_string()),
        ),
        (
            key("SnippetTagUUIDs"),
            Value::Array(vec![Value::String(tag_id.to_string())]),
        ),
        (
            key("SnippetDateCreated"),
            Value::String("2024-01-02T03:04:05Z".to_owned()),
        ),
        (
            key("SnippetDateModified"),
            Value::String("2024-02-03T04:05:06Z".to_owned()),
        ),
        (key("Pinned"), Value::Boolean(true)),
        (key("Locked"), Value::Boolean(true)),
        (key("SnippetParts"), Value::Array(vec![first, second])),
    ]);
    write_archive(&root.join("Database/Snippets/imported.data"), snippet);
    (snippet_id, first_fragment_id, second_fragment_id)
}

#[test]
fn imports_a_synthetic_library_through_staging_without_touching_source() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("legacy.snippetslablibrary");
    let destination = temporary.path().join("Imported.sniplib");
    let (snippet_id, first_fragment_id, second_fragment_id) = legacy_fixture(&source);
    let before = fs::read(source.join("Database/Snippets/imported.data")).unwrap();

    let dry_run = import_snippetslab(&source, &destination, true).unwrap();
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.snippets, 1);
    assert_eq!(dry_run.folders, 2);
    assert_eq!(dry_run.tags, 1);
    assert_eq!(dry_run.fragments, 2);
    assert_eq!(dry_run.notes, 1);
    assert_eq!(dry_run.attachments, 1);
    assert_eq!(dry_run.normalized_tags, vec!["\" dev \" -> \"dev\""]);
    assert!(dry_run.warnings[0].contains("attachment"));
    assert!(!destination.exists());

    let report = import_snippetslab(&source, &destination, false).unwrap();
    assert!(!report.dry_run);
    assert_eq!(
        before,
        fs::read(source.join("Database/Snippets/imported.data")).unwrap()
    );
    let library = Library::open(&destination).unwrap();
    let catalog = library.scan().unwrap();
    assert_eq!(catalog.folders, vec!["Parent", "Parent/Child"]);
    assert_eq!(catalog.tags, vec!["dev"]);
    let snippet = library
        .resolve_snippet(&catalog, &snippet_id.to_string())
        .unwrap();
    assert_eq!(snippet.title, "Imported snippet");
    assert_eq!(snippet.folder, "Parent/Child");
    assert_eq!(snippet.tags, vec!["dev"]);
    assert!(snippet.pinned);
    assert!(snippet.locked);
    assert_eq!(snippet.created_at, "2024-01-02T03:04:05Z");
    assert_eq!(
        snippet.source.as_ref().unwrap().modified_at.as_deref(),
        Some("2024-02-03T04:05:06Z")
    );
    assert_eq!(snippet.loaded_fragments.len(), 2);
    assert_eq!(snippet.loaded_fragments[0].id, first_fragment_id);
    assert_eq!(snippet.loaded_fragments[0].language, "bash");
    assert_eq!(
        snippet.loaded_fragments[0].source_language.as_deref(),
        Some("BashLexer")
    );
    assert_eq!(snippet.loaded_fragments[0].content, "echo imported\n");
    assert_eq!(
        snippet.loaded_fragments[0].note_content.as_deref(),
        Some("first note")
    );
    assert_eq!(snippet.loaded_fragments[1].id, second_fragment_id);
    assert_eq!(snippet.loaded_fragments[1].language, "markdown");
    assert!(doctor(&library, false).ok);
}

#[test]
fn language_mapping_and_nested_folder_paths_are_stable() {
    assert_eq!(map_language("FishShellLexer"), "fish");
    assert_eq!(map_language("UnknownLexer"), "text");
    let folders = vec![
        LegacyFolder {
            uuid: "parent".to_owned(),
            title: "Parent Folder".to_owned(),
            parent_uuid: None,
        },
        LegacyFolder {
            uuid: "child".to_owned(),
            title: "child/name".to_owned(),
            parent_uuid: Some("parent".to_owned()),
        },
    ];
    assert_eq!(
        build_folder_paths(&folders).get("child"),
        Some(&"Parent Folder/child-name".to_owned())
    );
}

const LIBRARY_ID: &str = "00000000-0000-0000-0000-000000000001";
const PARENT_ID: &str = "00000000-0000-0000-0000-000000000010";
const CHILD_ID: &str = "00000000-0000-0000-0000-000000000011";
const TAG_ID: &str = "00000000-0000-0000-0000-000000000020";
const SNIPPET_ID: &str = "00000000-0000-0000-0000-000000000030";
const FIRST_FRAGMENT_ID: &str = "00000000-0000-0000-0000-000000000031";
const SECOND_FRAGMENT_ID: &str = "00000000-0000-0000-0000-000000000032";

fn committed_fixture_content() -> Vec<(PathBuf, Value)> {
    let parent = dict(vec![(
        key("NodeDictRep"),
        dict(vec![
            (key("NodeType"), Value::Integer(3.into())),
            (
                key("NodeChildren"),
                Value::Array(vec![dict(vec![
                    (key("NodeType"), Value::Integer(3.into())),
                    (key("NodeChildren"), Value::Array(Vec::new())),
                    (
                        key("NodeSystemSymbolName"),
                        Value::String("greaterthan".to_owned()),
                    ),
                    (key("NodeUUID"), Value::String(CHILD_ID.to_owned())),
                    (key("NodeTitle"), Value::String("Child".to_owned())),
                ])]),
            ),
            (
                key("NodeSystemSymbolName"),
                Value::String("greaterthan".to_owned()),
            ),
            (key("NodeUUID"), Value::String(PARENT_ID.to_owned())),
            (key("NodeTitle"), Value::String("Parent".to_owned())),
        ]),
    )]);
    let tag = dict(vec![
        (key("TagUUID"), Value::String(TAG_ID.to_owned())),
        (key("TagTitle"), Value::String(" dev ".to_owned())),
        (key("TagColor"), Value::Integer(3.into())),
    ]);
    let first = dict(vec![
        (
            key("SnippetPartUUID"),
            Value::String(FIRST_FRAGMENT_ID.to_owned()),
        ),
        (key("SnippetPartTitle"), Value::String("run.sh".to_owned())),
        (
            key("SnippetPartLanguage"),
            Value::String("BashLexer".to_owned()),
        ),
        (
            key("SnippetPartContent"),
            Value::Data(b"echo fixture\n".to_vec()),
        ),
        (
            key("SnippetPartNote"),
            Value::Data(b"fixture note".to_vec()),
        ),
    ]);
    let second = dict(vec![
        (
            key("SnippetPartUUID"),
            Value::String(SECOND_FRAGMENT_ID.to_owned()),
        ),
        (key("SnippetPartTitle"), Value::String("readme".to_owned())),
        (
            key("SnippetPartLanguage"),
            Value::String("MarkdownLexer".to_owned()),
        ),
        (
            key("SnippetPartContent"),
            Value::Data(b"# Fixture\n".to_vec()),
        ),
        (key("SnippetPartNote"), Value::Data(Vec::new())),
    ]);
    let snippet = dict(vec![
        (key("SnippetUUID"), Value::String(SNIPPET_ID.to_owned())),
        (
            key("SnippetTitle"),
            Value::String("Fixture snippet".to_owned()),
        ),
        (key("SnippetFolderUUID"), Value::String(CHILD_ID.to_owned())),
        (
            key("SnippetTagUUIDs"),
            Value::Array(vec![Value::String(TAG_ID.to_owned())]),
        ),
        (
            key("SnippetDateCreated"),
            Value::String("2024-01-02T03:04:05Z".to_owned()),
        ),
        (
            key("SnippetDateModified"),
            Value::String("2024-02-03T04:05:06Z".to_owned()),
        ),
        (key("Pinned"), Value::Boolean(true)),
        (key("Locked"), Value::Boolean(false)),
        (key("SnippetParts"), Value::Array(vec![first, second])),
    ]);
    vec![
        (
            PathBuf::from("identifier"),
            Value::String(LIBRARY_ID.to_owned()),
        ),
        (
            PathBuf::from("version.plist"),
            dict(vec![(
                "SnippetsLab".to_owned(),
                Value::String("2.6".to_owned()),
            )]),
        ),
        (
            PathBuf::from("Database/folders.data"),
            Value::Array(vec![Value::Data(archive(parent))]),
        ),
        (
            PathBuf::from("Database/tags.data"),
            Value::Array(vec![Value::Data(archive(tag))]),
        ),
        (
            PathBuf::from(format!("Database/Snippets/{SNIPPET_ID}.data")),
            snippet,
        ),
    ]
}

fn write_committed_fixture(root: &Path) {
    fs::create_dir_all(root.join("Database/Snippets")).unwrap();
    for (path, value) in committed_fixture_content() {
        let destination = root.join(&path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        write_archive(&destination, value);
    }
}

#[test]
#[ignore = "regenerate the committed fixture; not run in CI"]
fn regenerate_committed_fixture() {
    let root = committed_fixture_root();
    let _ = fs::remove_dir_all(&root);
    write_committed_fixture(&root);
    assert!(root.join("Database/Snippets").is_dir());
}

#[test]
fn imports_the_committed_fixture() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("snippetslab-2.6.snippetslablibrary");
    copy_dir_recursive(&committed_fixture_root(), &source);
    let destination = temporary.path().join("Imported.sniplib");

    let dry_run = import_snippetslab(&source, &destination, true).unwrap();
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.library_id, LIBRARY_ID);
    assert_eq!(dry_run.format_version, "2.6");
    assert_eq!(dry_run.snippets, 1);
    assert_eq!(dry_run.folders, 2);
    assert_eq!(dry_run.tags, 1);
    assert_eq!(dry_run.fragments, 2);
    assert_eq!(dry_run.notes, 1);
    assert_eq!(dry_run.attachments, 0);
    assert!(dry_run.warnings.is_empty());

    let report = import_snippetslab(&source, &destination, false).unwrap();
    assert!(!report.dry_run);
    let library = Library::open(&destination).unwrap();
    let catalog = library.scan().unwrap();
    assert_eq!(catalog.folders, vec!["Parent", "Parent/Child"]);
    assert_eq!(catalog.tags, vec!["dev"]);
    let snippet = library.resolve_snippet(&catalog, SNIPPET_ID).unwrap();
    assert_eq!(snippet.title, "Fixture snippet");
    assert_eq!(snippet.folder, "Parent/Child");
    assert_eq!(snippet.tags, vec!["dev"]);
    assert!(snippet.pinned);
    assert!(!snippet.locked);
    assert_eq!(snippet.created_at, "2024-01-02T03:04:05Z");
    assert_eq!(
        snippet.source.as_ref().unwrap().modified_at.as_deref(),
        Some("2024-02-03T04:05:06Z")
    );
    assert_eq!(snippet.loaded_fragments.len(), 2);
    assert_eq!(
        snippet.loaded_fragments[0].id,
        Uuid::parse_str(FIRST_FRAGMENT_ID).unwrap()
    );
    assert_eq!(snippet.loaded_fragments[0].language, "bash");
    assert_eq!(
        snippet.loaded_fragments[0].source_language.as_deref(),
        Some("BashLexer")
    );
    assert_eq!(snippet.loaded_fragments[0].content, "echo fixture\n");
    assert_eq!(
        snippet.loaded_fragments[0].note_content.as_deref(),
        Some("fixture note")
    );
    assert_eq!(
        snippet.loaded_fragments[1].id,
        Uuid::parse_str(SECOND_FRAGMENT_ID).unwrap()
    );
    assert_eq!(snippet.loaded_fragments[1].language, "markdown");
    assert!(doctor(&library, false).ok);
}
