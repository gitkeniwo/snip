use plist::{Dictionary, Uid, Value};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::domain::{SCHEMA_VERSION, SourceMetadata, TagDefinition, TagRegistry};
use crate::error::{Result, SnipError};
use crate::filesystem::{Library, safe_component};
use crate::service::{CreateOptions, FragmentAddOptions, add_fragment, create_snippet, doctor};

const KEY_PREFIX: &str = "com.renfei.SnippetsLab.Key.";
const APPLE_EPOCH_UNIX_SECONDS: f64 = 978_307_200.0;
const UNCATEGORIZED_UUID: &str = "com.renfei.SnippetsLab.UUID.Predefined.Uncategorized";

#[derive(Clone, Debug, Serialize)]
pub struct ImportReport {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub dry_run: bool,
    pub library_id: String,
    pub format_version: String,
    pub snippets: usize,
    pub folders: usize,
    pub tags: usize,
    pub fragments: usize,
    pub notes: usize,
    pub attachments: usize,
    pub normalized_tags: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
struct LegacyFolder {
    uuid: String,
    title: String,
    parent_uuid: Option<String>,
}

#[derive(Clone, Debug)]
struct LegacyTag {
    uuid: String,
    title: String,
    color: Option<i64>,
}

#[derive(Clone, Debug)]
struct LegacyPart {
    uuid: Option<String>,
    title: Option<String>,
    language: Option<String>,
    content: String,
    note: String,
}

#[derive(Clone, Debug)]
struct LegacySnippet {
    uuid: String,
    title: String,
    folder_uuid: Option<String>,
    tag_uuids: Vec<String>,
    created: Option<String>,
    modified: Option<String>,
    pinned: bool,
    locked: bool,
    parts: Vec<LegacyPart>,
}

pub fn import_snippetslab(
    source: &Path,
    destination: &Path,
    dry_run: bool,
) -> Result<ImportReport> {
    let source = LegacyLibrary::open(source)?;
    let library_id = source.identifier()?;
    let format_version = source.version()?;
    let folders = source.folders()?;
    let tags = source.tags()?;
    let snippets = source.snippets()?;
    let attachments = count_files(&source.root.join("Database/Attachments"))?;
    let folder_paths = build_folder_paths(&folders);
    let mut normalized_tags = Vec::new();
    let tag_map = tags
        .iter()
        .map(|tag| {
            let trimmed = tag.title.trim().to_owned();
            if trimmed != tag.title {
                normalized_tags.push(format!("{:?} -> {:?}", tag.title, trimmed));
            }
            (tag.uuid.clone(), trimmed)
        })
        .collect::<HashMap<_, _>>();
    let fragment_count = snippets.iter().map(|snippet| snippet.parts.len()).sum();
    let note_count = snippets
        .iter()
        .flat_map(|snippet| &snippet.parts)
        .filter(|part| !part.note.is_empty())
        .count();
    let mut warnings = Vec::new();
    if attachments > 0 {
        warnings.push(format!(
            "{attachments} attachment file(s) were found; attachment relationship import is not supported in schema v1"
        ));
    }
    let report = ImportReport {
        source: source.root.clone(),
        destination: destination.to_path_buf(),
        dry_run,
        library_id: library_id.clone(),
        format_version: format_version.clone(),
        snippets: snippets.len(),
        folders: folders.len(),
        tags: tags.len(),
        fragments: fragment_count,
        notes: note_count,
        attachments,
        normalized_tags,
        warnings,
    };
    if dry_run {
        return Ok(report);
    }
    if destination.exists() {
        return Err(SnipError::conflict(format!(
            "import destination already exists: {}",
            destination.display()
        )));
    }
    let parent = destination.parent().ok_or_else(|| {
        SnipError::usage(format!(
            "destination has no parent: {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent)?;
    let stage = parent.join(format!(".snip-import-{}", Uuid::new_v4().simple()));
    let imported = (|| -> Result<()> {
        let library = Library::init(
            &stage,
            destination.file_stem().and_then(|value| value.to_str()),
        )?;
        let registry = TagRegistry {
            schema_version: SCHEMA_VERSION,
            tags: tags
                .iter()
                .map(|tag| {
                    Ok(TagDefinition {
                        id: parse_uuid(&tag.uuid, "tag")?,
                        name: tag.title.trim().to_owned(),
                        color: tag.color,
                        source_id: Some(tag.uuid.clone()),
                        extra: toml::Table::new(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            extra: toml::Table::new(),
        };
        library.write_tag_registry(&registry)?;
        for folder in folder_paths.values() {
            if !folder.is_empty() {
                let path = library.snippets_dir().join(folder);
                fs::create_dir_all(&path)?;
                crate::filesystem::atomic_write(&path.join(".keep"), b"")?;
            }
        }
        for legacy in &snippets {
            let first = legacy.parts.first().cloned().unwrap_or(LegacyPart {
                uuid: None,
                title: Some("Fragment".to_owned()),
                language: Some("TextLexer".to_owned()),
                content: String::new(),
                note: String::new(),
            });
            let original_language = first
                .language
                .clone()
                .unwrap_or_else(|| "TextLexer".to_owned());
            let language = map_language(&original_language).to_owned();
            let tags = legacy
                .tag_uuids
                .iter()
                .map(|uuid| tag_map.get(uuid).cloned().unwrap_or_else(|| uuid.clone()))
                .collect::<Vec<_>>();
            let id = parse_uuid(&legacy.uuid, "snippet")?;
            let fragment_id = first
                .uuid
                .as_deref()
                .map(|value| parse_uuid(value, "fragment"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4);
            let snippet = create_snippet(
                &library,
                &CreateOptions {
                    id: Some(id),
                    fragment_id: Some(fragment_id),
                    title: legacy.title.clone(),
                    folder: legacy
                        .folder_uuid
                        .as_ref()
                        .filter(|uuid| uuid.as_str() != UNCATEGORIZED_UUID)
                        .and_then(|uuid| folder_paths.get(uuid).cloned()),
                    tags,
                    language,
                    source_language: Some(original_language),
                    fragment_title: first.title.clone(),
                    content: first.content,
                    note: (!first.note.is_empty()).then_some(first.note),
                    readme: None,
                    pinned: legacy.pinned,
                    locked: legacy.locked,
                    created_at: legacy.created.clone(),
                    source: Some(SourceMetadata {
                        kind: "snippetslab".to_owned(),
                        library_id: Some(library_id.clone()),
                        original_id: Some(legacy.uuid.clone()),
                        format_version: Some(format_version.clone()),
                        modified_at: legacy.modified.clone(),
                        extra: toml::Table::new(),
                    }),
                },
            )?;
            for part in legacy.parts.iter().skip(1) {
                let original_language = part
                    .language
                    .clone()
                    .unwrap_or_else(|| "TextLexer".to_owned());
                add_fragment(
                    &library,
                    &snippet.id.to_string(),
                    &FragmentAddOptions {
                        id: part
                            .uuid
                            .as_deref()
                            .map(|value| parse_uuid(value, "fragment"))
                            .transpose()?,
                        title: part.title.clone().unwrap_or_else(|| "Fragment".to_owned()),
                        language: map_language(&original_language).to_owned(),
                        source_language: Some(original_language),
                        content: part.content.clone(),
                        note: (!part.note.is_empty()).then_some(part.note.clone()),
                        if_hash: None,
                        force: true,
                    },
                )?;
            }
        }
        let validation = doctor(&library, false);
        if !validation.ok {
            return Err(SnipError::validation(format!(
                "imported library failed validation: {}",
                validation.errors.join("; ")
            )));
        }
        Ok(())
    })();
    if let Err(error) = imported {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    fs::rename(&stage, destination).map_err(|error| {
        let _ = fs::remove_dir_all(&stage);
        SnipError::io(format!(
            "cannot publish imported library {}: {error}",
            destination.display()
        ))
    })?;
    Ok(report)
}

struct LegacyLibrary {
    root: PathBuf,
}

impl LegacyLibrary {
    fn open(path: &Path) -> Result<Self> {
        let root = fs::canonicalize(path).map_err(|error| {
            SnipError::not_found(format!("SnippetsLab library does not exist: {error}"))
        })?;
        for required in [
            root.join("identifier"),
            root.join("version.plist"),
            root.join("Database/Snippets"),
        ] {
            if !required.exists() {
                return Err(SnipError::validation(format!(
                    "not a SnippetsLab library; missing {}",
                    required.display()
                )));
            }
        }
        Ok(Self { root })
    }

    fn decode_file(&self, path: &Path) -> Result<Decoded> {
        let bytes = fs::read(path)?;
        unarchive_bytes(&bytes).map_err(|error| {
            SnipError::validation(format!("cannot decode {}: {error}", path.display()))
        })
    }

    fn identifier(&self) -> Result<String> {
        Ok(self.decode_file(&self.root.join("identifier"))?.text())
    }

    fn version(&self) -> Result<String> {
        let value = self.decode_file(&self.root.join("version.plist"))?;
        Ok(value
            .as_dict()
            .and_then(|dict| dict.get("SnippetsLab"))
            .and_then(Decoded::as_str)
            .unwrap_or("unknown")
            .to_owned())
    }

    fn wrapped_items(&self, name: &str) -> Result<Vec<Decoded>> {
        let value = self.decode_file(&self.root.join("Database").join(name))?;
        let items = value.as_array().ok_or_else(|| {
            SnipError::validation(format!("Database/{name} does not contain an array"))
        })?;
        items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                Decoded::Data(data) => unarchive_bytes(data).map_err(|error| {
                    SnipError::validation(format!(
                        "cannot decode Database/{name} item {index}: {error}"
                    ))
                }),
                _ => Err(SnipError::validation(format!(
                    "Database/{name} item {index} is not archive data"
                ))),
            })
            .collect()
    }

    fn folders(&self) -> Result<Vec<LegacyFolder>> {
        fn walk(
            node: &BTreeMap<String, Decoded>,
            parent: Option<String>,
            output: &mut Vec<LegacyFolder>,
        ) {
            let representation = node
                .get(&key("NodeDictRep"))
                .and_then(Decoded::as_dict)
                .unwrap_or(node);
            let uuid = decoded_string(representation, &key("NodeUUID"));
            let title = decoded_string(representation, &key("NodeTitle"));
            let next_parent = match (uuid, title) {
                (Some(uuid), Some(title)) => {
                    output.push(LegacyFolder {
                        uuid: uuid.clone(),
                        title,
                        parent_uuid: parent,
                    });
                    Some(uuid)
                }
                _ => parent,
            };
            if let Some(children) = representation
                .get(&key("NodeChildren"))
                .and_then(Decoded::as_array)
            {
                for child in children {
                    if let Some(child) = child.as_dict() {
                        walk(child, next_parent.clone(), output);
                    }
                }
            }
        }
        let mut output = Vec::new();
        for item in self.wrapped_items("folders.data")? {
            let node = item
                .as_dict()
                .ok_or_else(|| SnipError::validation("folder archive root is not a dictionary"))?;
            walk(node, None, &mut output);
        }
        Ok(output)
    }

    fn tags(&self) -> Result<Vec<LegacyTag>> {
        self.wrapped_items("tags.data")?
            .into_iter()
            .map(|item| {
                let dict = item
                    .as_dict()
                    .ok_or_else(|| SnipError::validation("tag root is not a dictionary"))?;
                Ok(LegacyTag {
                    uuid: required_string(dict, &key("TagUUID"))?,
                    title: required_string(dict, &key("TagTitle"))?,
                    color: dict.get(&key("TagColor")).and_then(Decoded::as_i64),
                })
            })
            .collect()
    }

    fn snippets(&self) -> Result<Vec<LegacySnippet>> {
        let mut paths = fs::read_dir(self.root.join("Database/Snippets"))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("data"))
            .collect::<Vec<_>>();
        paths.sort();
        paths.iter().map(|path| self.snippet(path)).collect()
    }

    fn snippet(&self, path: &Path) -> Result<LegacySnippet> {
        let value = self.decode_file(path)?;
        let root = value
            .as_dict()
            .ok_or_else(|| SnipError::validation("snippet root is not a dictionary"))?;
        let uuid = required_string(root, &key("SnippetUUID"))?;
        let mut parts = Vec::new();
        if let Some(items) = root.get(&key("SnippetParts")).and_then(Decoded::as_array) {
            for item in items {
                let Some(part) = item.as_dict() else {
                    continue;
                };
                parts.push(LegacyPart {
                    uuid: decoded_string(part, &key("SnippetPartUUID")),
                    title: decoded_string(part, &key("SnippetPartTitle")),
                    language: decoded_string(part, &key("SnippetPartLanguage")),
                    content: part
                        .get(&key("SnippetPartContent"))
                        .map(Decoded::text)
                        .unwrap_or_default(),
                    note: part
                        .get(&key("SnippetPartNote"))
                        .map(Decoded::text)
                        .unwrap_or_default(),
                });
            }
        }
        Ok(LegacySnippet {
            uuid,
            title: required_string(root, &key("SnippetTitle"))?,
            folder_uuid: decoded_string(root, &key("SnippetFolderUUID")),
            tag_uuids: root
                .get(&key("SnippetTagUUIDs"))
                .and_then(Decoded::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Decoded::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            created: decoded_string(root, &key("SnippetDateCreated")),
            modified: decoded_string(root, &key("SnippetDateModified")),
            pinned: root
                .get(&key("Pinned"))
                .and_then(Decoded::as_bool)
                .unwrap_or(false),
            locked: root
                .get(&key("Locked"))
                .and_then(Decoded::as_bool)
                .unwrap_or(false),
            parts,
        })
    }
}

#[derive(Clone, Debug)]
enum Decoded {
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Real(f64),
    String(String),
    Data(Vec<u8>),
    Date(String),
    Array(Vec<Decoded>),
    Dict(BTreeMap<String, Decoded>),
}

impl Decoded {
    fn as_dict(&self) -> Option<&BTreeMap<String, Decoded>> {
        match self {
            Self::Dict(value) => Some(value),
            _ => None,
        }
    }
    fn as_array(&self) -> Option<&[Decoded]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }
    fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) | Self::Date(value) => Some(value),
            _ => None,
        }
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }
    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Signed(value) => Some(*value),
            Self::Unsigned(value) => i64::try_from(*value).ok(),
            _ => None,
        }
    }
    fn text(&self) -> String {
        match self {
            Self::Data(value) => String::from_utf8_lossy(value).into_owned(),
            Self::String(value) | Self::Date(value) => value.clone(),
            Self::Null => String::new(),
            Self::Bool(value) => value.to_string(),
            Self::Signed(value) => value.to_string(),
            Self::Unsigned(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
            Self::Array(_) | Self::Dict(_) => String::new(),
        }
    }
}

struct Decoder<'a> {
    objects: &'a [Value],
    active: HashSet<usize>,
}

impl<'a> Decoder<'a> {
    fn decode(&mut self, value: &Value) -> Result<Decoded> {
        match value {
            Value::Uid(uid) => self.decode_index(uid_to_index(uid)?),
            Value::Array(values) => values
                .iter()
                .map(|value| self.decode(value))
                .collect::<Result<Vec<_>>>()
                .map(Decoded::Array),
            Value::Dictionary(dict) => self.decode_plain_dict(dict),
            Value::Boolean(value) => Ok(Decoded::Bool(*value)),
            Value::Data(value) => Ok(Decoded::Data(value.clone())),
            Value::Date(value) => Ok(Decoded::Date(value.to_xml_format())),
            Value::Real(value) => Ok(Decoded::Real(*value)),
            Value::Integer(value) => {
                if let Some(value) = value.as_signed() {
                    Ok(Decoded::Signed(value))
                } else if let Some(value) = value.as_unsigned() {
                    Ok(Decoded::Unsigned(value))
                } else {
                    Err(SnipError::validation("unsupported plist integer"))
                }
            }
            Value::String(value) if value == "$null" => Ok(Decoded::Null),
            Value::String(value) => Ok(Decoded::String(value.clone())),
            _ => Err(SnipError::validation("unsupported plist value")),
        }
    }

    fn decode_index(&mut self, index: usize) -> Result<Decoded> {
        if index == 0 {
            return Ok(Decoded::Null);
        }
        let value = self
            .objects
            .get(index)
            .ok_or_else(|| SnipError::validation(format!("out-of-range archive UID {index}")))?;
        if !self.active.insert(index) {
            return Ok(Decoded::Null);
        }
        let result = match value {
            Value::Dictionary(dict) => self.decode_object_dict(dict),
            _ => self.decode(value),
        };
        self.active.remove(&index);
        result
    }

    fn decode_object_dict(&mut self, dict: &Dictionary) -> Result<Decoded> {
        let class = class_name(self.objects, dict)?;
        match class.as_deref() {
            Some("NSArray" | "NSMutableArray" | "NSSet" | "NSMutableSet") => dict
                .get("NS.objects")
                .and_then(Value::as_array)
                .ok_or_else(|| SnipError::validation("Foundation array is missing NS.objects"))?
                .iter()
                .map(|value| self.decode(value))
                .collect::<Result<Vec<_>>>()
                .map(Decoded::Array),
            Some("NSDictionary" | "NSMutableDictionary") => {
                let keys = dict
                    .get("NS.keys")
                    .and_then(Value::as_array)
                    .ok_or_else(|| SnipError::validation("NSDictionary is missing NS.keys"))?;
                let values = dict
                    .get("NS.objects")
                    .and_then(Value::as_array)
                    .ok_or_else(|| SnipError::validation("NSDictionary is missing NS.objects"))?;
                let mut result = BTreeMap::new();
                for (key, value) in keys.iter().zip(values) {
                    let key = self.decode(key)?;
                    let key = key
                        .as_str()
                        .ok_or_else(|| SnipError::validation("dictionary key is not text"))?;
                    result.insert(key.to_owned(), self.decode(value)?);
                }
                Ok(Decoded::Dict(result))
            }
            Some("NSData" | "NSMutableData") => match dict.get("NS.data") {
                Some(Value::Data(value)) => Ok(Decoded::Data(value.clone())),
                Some(value) => self.decode(value),
                None => Ok(Decoded::Data(Vec::new())),
            },
            Some("NSDate") => {
                let seconds = dict
                    .get("NS.time")
                    .and_then(number_value)
                    .ok_or_else(|| SnipError::validation("NSDate is missing NS.time"))?;
                Ok(Decoded::Date(ns_time_to_rfc3339(seconds)?))
            }
            _ => {
                let mut result = BTreeMap::new();
                if let Some(class) = class {
                    result.insert("__class".to_owned(), Decoded::String(class));
                }
                for (key, value) in dict {
                    if key != "$class" {
                        result.insert(key.clone(), self.decode(value)?);
                    }
                }
                Ok(Decoded::Dict(result))
            }
        }
    }

    fn decode_plain_dict(&mut self, dict: &Dictionary) -> Result<Decoded> {
        if dict.contains_key("$class") {
            return self.decode_object_dict(dict);
        }
        let mut result = BTreeMap::new();
        for (key, value) in dict {
            result.insert(key.clone(), self.decode(value)?);
        }
        Ok(Decoded::Dict(result))
    }
}

fn unarchive_bytes(data: &[u8]) -> Result<Decoded> {
    let value = Value::from_reader(Cursor::new(data))
        .map_err(|error| SnipError::validation(format!("invalid property list: {error}")))?;
    let archive = value
        .as_dictionary()
        .ok_or_else(|| SnipError::validation("archive root is not a dictionary"))?;
    if archive.get("$archiver").and_then(Value::as_string) != Some("NSKeyedArchiver") {
        return Err(SnipError::validation(
            "plist is not an NSKeyedArchiver archive",
        ));
    }
    let objects = archive
        .get("$objects")
        .and_then(Value::as_array)
        .ok_or_else(|| SnipError::validation("archive is missing $objects"))?;
    let root = archive
        .get("$top")
        .and_then(Value::as_dictionary)
        .and_then(|top| top.get("root"))
        .and_then(Value::as_uid)
        .ok_or_else(|| SnipError::validation("archive is missing root UID"))?;
    Decoder {
        objects,
        active: HashSet::new(),
    }
    .decode_index(uid_to_index(root)?)
}

fn class_name(objects: &[Value], dict: &Dictionary) -> Result<Option<String>> {
    let Some(Value::Uid(reference)) = dict.get("$class") else {
        return Ok(None);
    };
    let index = uid_to_index(reference)?;
    Ok(objects
        .get(index)
        .and_then(Value::as_dictionary)
        .and_then(|class| class.get("$classname"))
        .and_then(Value::as_string)
        .map(str::to_owned))
}

fn uid_to_index(uid: &Uid) -> Result<usize> {
    usize::try_from(uid.get()).map_err(|_| SnipError::validation("archive UID is too large"))
}

fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Real(value) => Some(*value),
        Value::Integer(value) => value
            .as_signed()
            .map(|value| value as f64)
            .or_else(|| value.as_unsigned().map(|value| value as f64)),
        _ => None,
    }
}

fn ns_time_to_rfc3339(seconds: f64) -> Result<String> {
    let nanos = ((seconds + APPLE_EPOCH_UNIX_SECONDS) * 1_000_000_000.0).round() as i128;
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .map_err(|error| SnipError::validation(format!("invalid NSDate: {error}")))?
        .format(&Rfc3339)
        .map_err(|error| SnipError::validation(format!("cannot format NSDate: {error}")))
}

fn build_folder_paths(folders: &[LegacyFolder]) -> HashMap<String, String> {
    fn resolve(
        folder: &LegacyFolder,
        by_id: &HashMap<String, &LegacyFolder>,
        cache: &mut HashMap<String, String>,
    ) -> String {
        if let Some(path) = cache.get(&folder.uuid) {
            return path.clone();
        }
        let component = safe_component(&folder.title);
        let path = folder
            .parent_uuid
            .as_ref()
            .and_then(|parent| by_id.get(parent))
            .map(|parent| format!("{}/{}", resolve(parent, by_id, cache), component))
            .unwrap_or(component);
        cache.insert(folder.uuid.clone(), path.clone());
        path
    }
    let by_id = folders
        .iter()
        .map(|folder| (folder.uuid.clone(), folder))
        .collect::<HashMap<_, _>>();
    let mut cache = HashMap::new();
    for folder in folders {
        resolve(folder, &by_id, &mut cache);
    }
    cache
}

fn map_language(value: &str) -> &'static str {
    match value {
        "BashLexer" => "bash",
        "MarkdownLexer" => "markdown",
        "PythonLexer" => "python",
        "FishShellLexer" => "fish",
        "SqlLexer" => "sql",
        "JsonLexer" => "json",
        "YamlLexer" => "yaml",
        "CssLexer" => "css",
        "DockerLexer" => "dockerfile",
        "HtmlLexer" => "html",
        "JavascriptLexer" => "javascript",
        "MakefileLexer" => "makefile",
        "TclLexer" => "tcl",
        "UnixConfigLexer" => "text",
        _ => "text",
    }
}

fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

fn decoded_string(dict: &BTreeMap<String, Decoded>, field: &str) -> Option<String> {
    dict.get(field).and_then(Decoded::as_str).map(str::to_owned)
}

fn required_string(dict: &BTreeMap<String, Decoded>, field: &str) -> Result<String> {
    decoded_string(dict, field)
        .ok_or_else(|| SnipError::validation(format!("missing string field {field}")))
}

fn parse_uuid(value: &str, kind: &str) -> Result<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| SnipError::validation(format!("invalid {kind} UUID {value:?}: {error}")))
}

fn count_files(path: &Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    fn walk(path: &Path, count: &mut usize) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), count)?;
            } else if entry.file_type()?.is_file() {
                *count += 1;
            }
        }
        Ok(())
    }
    let mut count = 0;
    walk(path, &mut count)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
