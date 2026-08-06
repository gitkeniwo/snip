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

/// Builds a faithful NSKeyedArchiver object graph: `$objects` with shared
/// sub-objects addressed by `UID`, class dictionaries carrying `$classname`,
/// and Foundation wrappers (`NS.data`, `NS.time`, `NS.objects`, `NS.keys`) —
/// exactly the shape SnippetsLab 2.6 writes. Object 0 is `$null`; index 1 is
/// the root, mirroring the real archives.
struct ArchiveBuilder {
    objects: Vec<Value>,
}

impl ArchiveBuilder {
    fn new() -> Self {
        Self {
            objects: vec![Value::String("$null".to_owned())],
        }
    }

    fn push(&mut self, value: Value) -> usize {
        self.objects.push(value);
        self.objects.len() - 1
    }

    fn uid(&self, index: usize) -> Value {
        Value::Uid(Uid::new(index as u64))
    }

    fn class(&mut self, name: &str, supers: &[&str]) -> usize {
        let mut classes = vec![name.to_owned()];
        classes.extend(supers.iter().map(|s| s.to_string()));
        self.push(dict(vec![
            ("$classname".to_owned(), Value::String(name.to_owned())),
            (
                "$classes".to_owned(),
                Value::Array(classes.into_iter().map(Value::String).collect()),
            ),
        ]))
    }

    fn ns_data(&mut self, data_class: usize, bytes: Vec<u8>) -> usize {
        self.push(dict(vec![
            ("$class".to_owned(), self.uid(data_class)),
            ("NS.data".to_owned(), Value::Data(bytes)),
        ]))
    }

    fn ns_date(&mut self, date_class: usize, seconds: f64) -> usize {
        self.push(dict(vec![
            ("$class".to_owned(), self.uid(date_class)),
            ("NS.time".to_owned(), Value::Real(seconds)),
        ]))
    }

    fn ns_array(&mut self, array_class: usize, items: Vec<usize>) -> usize {
        self.push(dict(vec![
            ("$class".to_owned(), self.uid(array_class)),
            (
                "NS.objects".to_owned(),
                Value::Array(items.into_iter().map(|i| self.uid(i)).collect()),
            ),
        ]))
    }

    fn ns_dictionary(&mut self, dict_class: usize, keys: Vec<usize>, values: Vec<usize>) -> usize {
        self.push(dict(vec![
            ("$class".to_owned(), self.uid(dict_class)),
            (
                "NS.keys".to_owned(),
                Value::Array(keys.into_iter().map(|i| self.uid(i)).collect()),
            ),
            (
                "NS.objects".to_owned(),
                Value::Array(values.into_iter().map(|i| self.uid(i)).collect()),
            ),
        ]))
    }

    fn object(&mut self, class: usize, fields: Vec<(String, usize)>) -> usize {
        let mut entries = fields
            .into_iter()
            .map(|(key, value)| (key, self.uid(value)))
            .collect::<Vec<_>>();
        entries.push(("$class".to_owned(), self.uid(class)));
        self.push(dict(entries))
    }

    fn object_raw(&mut self, class: usize, fields: Vec<(String, Value)>) -> usize {
        let mut entries = fields;
        entries.push(("$class".to_owned(), self.uid(class)));
        self.push(dict(entries))
    }

    fn finish(&self, root: usize) -> Vec<u8> {
        let mut top = Dictionary::new();
        top.insert("root".to_owned(), Value::Uid(Uid::new(root as u64)));
        let mut archive = Dictionary::new();
        archive.insert(
            "$archiver".to_owned(),
            Value::String("NSKeyedArchiver".to_owned()),
        );
        archive.insert("$objects".to_owned(), Value::Array(self.objects.clone()));
        archive.insert("$top".to_owned(), Value::Dictionary(top));
        let mut output = Vec::new();
        Value::Dictionary(archive)
            .to_writer_binary(&mut output)
            .unwrap();
        output
    }
}

fn folder_node(
    builder: &mut ArchiveBuilder,
    ns_dict_class: usize,
    ns_array_class: usize,
    node_class: usize,
    uuid: &str,
    title: &str,
    children: Vec<usize>,
) -> usize {
    let type_key = builder.push(Value::String(key("NodeType")));
    let children_key = builder.push(Value::String(key("NodeChildren")));
    let symbol_key = builder.push(Value::String(key("NodeSystemSymbolName")));
    let uuid_key = builder.push(Value::String(key("NodeUUID")));
    let title_key = builder.push(Value::String(key("NodeTitle")));
    let type_value = builder.push(Value::Integer(3.into()));
    let children_value = builder.ns_array(ns_array_class, children);
    let symbol_value = builder.push(Value::String("greaterthan".to_owned()));
    let uuid_value = builder.push(Value::String(uuid.to_owned()));
    let title_value = builder.push(Value::String(title.to_owned()));
    let representation = builder.ns_dictionary(
        ns_dict_class,
        vec![type_key, children_key, symbol_key, uuid_key, title_key],
        vec![
            type_value,
            children_value,
            symbol_value,
            uuid_value,
            title_value,
        ],
    );
    builder.object(node_class, vec![(key("NodeDictRep"), representation)])
}

#[allow(clippy::too_many_arguments)]
fn snippet_part(
    builder: &mut ArchiveBuilder,
    ns_array_class: usize,
    ns_data_class: usize,
    ns_date_class: usize,
    part_class: usize,
    snippet_uuid: usize,
    part_uuid: &str,
    title: &str,
    language: &str,
    content: Vec<u8>,
    note: Vec<u8>,
) -> usize {
    let attachments_value = builder.ns_array(ns_array_class, Vec::new());
    let content_value = builder.ns_data(ns_data_class, content);
    let created_value = builder.ns_date(ns_date_class, 723_397_772.0);
    let modified_value = builder.ns_date(ns_date_class, 723_503_216.0);
    let language_value = builder.push(Value::String(language.to_owned()));
    let note_value = builder.ns_data(ns_data_class, note);
    let attributes_value = builder.ns_data(ns_data_class, b"[]".to_vec());
    let title_value = builder.push(Value::String(title.to_owned()));
    let uuid_value = builder.push(Value::String(part_uuid.to_owned()));
    builder.object(
        part_class,
        vec![
            (key("SnippetPartAttachments"), attachments_value),
            (key("SnippetPartContent"), content_value),
            (key("SnippetPartDateCreated"), created_value),
            (key("SnippetPartDateModified"), modified_value),
            (key("SnippetPartLanguage"), language_value),
            (key("SnippetPartNote"), note_value),
            (key("SnippetPartNotesAttributes"), attributes_value),
            (key("SnippetPartSnippetUUID"), snippet_uuid),
            (key("SnippetPartTitle"), title_value),
            (key("SnippetPartUUID"), uuid_value),
        ],
    )
}

const LIBRARY_ID: &str = "00000000-0000-0000-0000-000000000001";
const PARENT_ID: &str = "00000000-0000-0000-0000-000000000010";
const CHILD_ID: &str = "00000000-0000-0000-0000-000000000011";
const TAG_ID: &str = "00000000-0000-0000-0000-000000000020";
const SNIPPET_ID: &str = "00000000-0000-0000-0000-000000000030";
const FIRST_FRAGMENT_ID: &str = "00000000-0000-0000-0000-000000000031";
const SECOND_FRAGMENT_ID: &str = "00000000-0000-0000-0000-000000000032";

fn snippet_archive() -> Vec<u8> {
    let mut builder = ArchiveBuilder::new();
    let ns_array_class = builder.class("NSArray", &["NSObject"]);
    let ns_data_class = builder.class("NSMutableData", &["NSData", "NSObject"]);
    let ns_date_class = builder.class("NSDate", &["NSObject"]);
    let snippet_class = builder.class("SLSnippet", &["NSObject"]);
    let part_class = builder.class("SLSnippetPart", &["NSObject"]);

    let title = builder.push(Value::String("Fixture snippet".to_owned()));
    let snippet_uuid = builder.push(Value::String(SNIPPET_ID.to_owned()));
    let folder_uuid = builder.push(Value::String(CHILD_ID.to_owned()));
    let tag_uuid = builder.push(Value::String(TAG_ID.to_owned()));
    let tag_uuids = builder.ns_array(ns_array_class, vec![tag_uuid]);
    let created = builder.ns_date(ns_date_class, 723_397_772.0);
    let modified = builder.ns_date(ns_date_class, 771_024_957.0);

    let first = snippet_part(
        &mut builder,
        ns_array_class,
        ns_data_class,
        ns_date_class,
        part_class,
        snippet_uuid,
        FIRST_FRAGMENT_ID,
        "run.sh",
        "BashLexer",
        b"echo fixture\n".to_vec(),
        b"fixture note".to_vec(),
    );
    let second = snippet_part(
        &mut builder,
        ns_array_class,
        ns_data_class,
        ns_date_class,
        part_class,
        snippet_uuid,
        SECOND_FRAGMENT_ID,
        "readme",
        "MarkdownLexer",
        b"# Fixture\n".to_vec(),
        Vec::new(),
    );
    let parts = builder.ns_array(ns_array_class, vec![first, second]);

    let root = builder.object_raw(
        snippet_class,
        vec![
            (key("DateDeleted"), builder.uid(0)),
            (key("GistIdentifier"), builder.uid(0)),
            (key("GitHubHTMLURL"), builder.uid(0)),
            (key("GitHubUsername"), builder.uid(0)),
            (key("Locked"), Value::Boolean(true)),
            (key("Pinned"), Value::Boolean(true)),
            (key("SnippetDateCreated"), builder.uid(created)),
            (key("SnippetDateModified"), builder.uid(modified)),
            (key("SnippetFolderUUID"), builder.uid(folder_uuid)),
            (key("SnippetParts"), builder.uid(parts)),
            (key("SnippetTagUUIDs"), builder.uid(tag_uuids)),
            (key("SnippetTitle"), builder.uid(title)),
            (key("SnippetUUID"), builder.uid(snippet_uuid)),
        ],
    );
    builder.finish(root)
}

fn folders_archive() -> Vec<u8> {
    let mut builder = ArchiveBuilder::new();
    let ns_array_class = builder.class("NSMutableArray", &["NSArray", "NSObject"]);
    let ns_dict_class = builder.class("NSDictionary", &["NSObject"]);
    let node_class = builder.class("SLCategoryNode", &["NSTreeNode", "NSObject"]);

    let child = folder_node(
        &mut builder,
        ns_dict_class,
        ns_array_class,
        node_class,
        CHILD_ID,
        "Child",
        Vec::new(),
    );
    let parent = folder_node(
        &mut builder,
        ns_dict_class,
        ns_array_class,
        node_class,
        PARENT_ID,
        "Parent",
        vec![child],
    );
    let parent_bytes = builder.finish(parent);
    let mut outer = ArchiveBuilder::new();
    let outer_array = outer.class("NSMutableArray", &["NSArray", "NSObject"]);
    let outer_data = outer.class("NSMutableData", &["NSData", "NSObject"]);
    let parent_item = outer.ns_data(outer_data, parent_bytes);
    let root = outer.ns_array(outer_array, vec![parent_item]);
    outer.finish(root)
}

fn tags_archive() -> Vec<u8> {
    let mut builder = ArchiveBuilder::new();
    let ns_array_class = builder.class("NSMutableArray", &["NSArray", "NSObject"]);
    let ns_data_class = builder.class("NSMutableData", &["NSData", "NSObject"]);

    let mut tag = ArchiveBuilder::new();
    let inner_tag_class = tag.class("SLTag", &["NSObject"]);
    let title = tag.push(Value::String(" dev ".to_owned()));
    let uuid = tag.push(Value::String(TAG_ID.to_owned()));
    let root = tag.object_raw(
        inner_tag_class,
        vec![
            (key("TagUUID"), tag.uid(uuid)),
            (key("TagTitle"), tag.uid(title)),
            (key("TagColor"), Value::Integer(0.into())),
        ],
    );
    let item = builder.ns_data(ns_data_class, tag.finish(root));
    let outer = builder.ns_array(ns_array_class, vec![item]);
    builder.finish(outer)
}

fn version_archive() -> Vec<u8> {
    let mut builder = ArchiveBuilder::new();
    let ns_dict_class = builder.class("NSDictionary", &["NSObject"]);
    let key_object = builder.push(Value::String("SnippetsLab".to_owned()));
    let value_object = builder.push(Value::String("2.6".to_owned()));
    let root = builder.ns_dictionary(ns_dict_class, vec![key_object], vec![value_object]);
    builder.finish(root)
}

fn identifier_archive() -> Vec<u8> {
    let mut builder = ArchiveBuilder::new();
    let root = builder.push(Value::String(LIBRARY_ID.to_owned()));
    builder.finish(root)
}

fn committed_fixture_content() -> Vec<(PathBuf, Vec<u8>)> {
    vec![
        (PathBuf::from("identifier"), identifier_archive()),
        (PathBuf::from("version.plist"), version_archive()),
        (PathBuf::from("Database/folders.data"), folders_archive()),
        (PathBuf::from("Database/tags.data"), tags_archive()),
        (
            PathBuf::from(format!("Database/Snippets/{SNIPPET_ID}.data")),
            snippet_archive(),
        ),
        (
            PathBuf::from("Database/Attachments/legacy.bin"),
            b"attachment".to_vec(),
        ),
    ]
}

fn write_committed_fixture(root: &Path) {
    for (path, bytes) in committed_fixture_content() {
        let destination = root.join(&path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, bytes).unwrap();
    }
}

#[test]
fn committed_fixture_matches_the_builder() {
    let root = committed_fixture_root();
    let mismatched = fixture_mismatches(&root);
    if mismatched.is_empty() {
        return;
    }
    if std::env::var_os("SNIP_REGENERATE_FIXTURE").is_some() {
        let expected = committed_fixture_content();
        let mut committed_paths = Vec::new();
        collect_relative_paths(&root, &root, &mut committed_paths);
        for path in &committed_paths {
            if !expected
                .iter()
                .any(|(expected_path, _)| expected_path == path)
            {
                let _ = fs::remove_file(root.join(path));
            }
        }
        write_committed_fixture(&root);
        assert!(
            fixture_mismatches(&root).is_empty(),
            "regeneration left the fixture out of sync"
        );
    } else {
        panic!(
            "committed fixture is out of sync with the builder for {mismatched:?}; \
             re-run with SNIP_REGENERATE_FIXTURE=1 to overwrite: \
             `SNIP_REGENERATE_FIXTURE=1 cargo test --lib committed_fixture_matches_the_builder`"
        );
    }
}

fn fixture_mismatches(root: &Path) -> Vec<PathBuf> {
    let expected = committed_fixture_content();
    let mut mismatched = expected
        .iter()
        .filter(|(path, bytes)| match fs::read(root.join(path)) {
            Ok(actual) => actual != *bytes,
            Err(_) => true,
        })
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();

    let mut committed_paths = Vec::new();
    collect_relative_paths(root, root, &mut committed_paths);
    for path in &committed_paths {
        if !expected
            .iter()
            .any(|(expected_path, _)| expected_path == path)
        {
            mismatched.push(path.clone());
        }
    }
    for (path, _) in &expected {
        if !committed_paths.contains(path) {
            mismatched.push(path.clone());
        }
    }
    mismatched
}

fn collect_relative_paths(dir: &Path, base: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_relative_paths(&path, base, paths);
        } else {
            paths.push(path.strip_prefix(base).unwrap().to_path_buf());
        }
    }
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
    assert_eq!(dry_run.attachments, 1);
    assert_eq!(dry_run.normalized_tags, vec!["\" dev \" -> \"dev\""]);
    assert!(dry_run.warnings[0].contains("attachment"));
    assert!(!destination.exists());

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
    assert!(snippet.locked);
    assert_eq!(snippet.created_at, "2023-12-04T15:49:32Z");
    assert_eq!(
        snippet.source.as_ref().unwrap().modified_at.as_deref(),
        Some("2025-06-07T21:35:57Z")
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
