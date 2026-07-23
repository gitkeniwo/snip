use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub const FORMAT_NAME: &str = "snip-library";
pub const SCHEMA_VERSION: u32 = 1;

/// Display name for snippets that live at the library root with no folder.
/// Every surface (CLI output, HTML preview, TUI) uses this one label.
pub const UNCATEGORIZED: &str = "Uncategorized";

/// Human-facing folder name: the folder path itself, or [`UNCATEGORIZED`] at the root.
pub fn folder_label(folder: &str) -> &str {
    if folder.is_empty() {
        UNCATEGORIZED
    } else {
        folder
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryManifest {
    pub format: String,
    pub schema_version: u32,
    pub id: Uuid,
    pub name: String,
    pub created_at: String,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagDefinition {
    pub id: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub tags: Vec<TagDefinition>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceMetadata {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FragmentManifest {
    pub id: Uuid,
    pub title: String,
    pub language: String,
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnippetManifest {
    pub schema_version: u32,
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub locked: bool,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceMetadata>,
    pub fragments: Vec<FragmentManifest>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fingerprint(pub String);

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Fragment {
    #[serde(flatten)]
    pub manifest: FragmentManifest,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_content: Option<String>,
    pub absolute_path: PathBuf,
}

impl std::ops::Deref for Fragment {
    type Target = FragmentManifest;

    fn deref(&self) -> &Self::Target {
        &self.manifest
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Snippet {
    #[serde(flatten)]
    pub manifest: SnippetManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    pub folder: String,
    pub package_path: PathBuf,
    pub modified_at: Option<String>,
    pub fingerprint: Fingerprint,
    pub loaded_fragments: Vec<Fragment>,
}

impl std::ops::Deref for Snippet {
    type Target = SnippetManifest;

    fn deref(&self) -> &Self::Target {
        &self.manifest
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogSnapshot {
    pub library: LibraryManifest,
    pub root: PathBuf,
    pub snippets: Vec<Snippet>,
    pub folders: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ChangeSet {
    pub fields: Vec<String>,
    pub old_fingerprint: Option<Fingerprint>,
    pub new_fingerprint: Option<Fingerprint>,
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResult {
    pub snippet_id: Uuid,
    pub title: String,
    pub folder: String,
    pub fragment_id: Option<Uuid>,
    pub fragment_title: Option<String>,
    pub line: Option<usize>,
    pub excerpt: String,
    pub score: u32,
}
