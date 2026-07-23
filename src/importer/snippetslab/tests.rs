use plist::{Dictionary, Uid, Value};
use std::fs;
use std::path::Path;
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
