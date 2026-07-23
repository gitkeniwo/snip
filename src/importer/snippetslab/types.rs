use serde::Serialize;
use std::path::PathBuf;

pub const KEY_PREFIX: &str = "com.renfei.SnippetsLab.Key.";
pub const APPLE_EPOCH_UNIX_SECONDS: f64 = 978_307_200.0;
pub const UNCATEGORIZED_UUID: &str = "com.renfei.SnippetsLab.UUID.Predefined.Uncategorized";

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
pub(crate) struct LegacyFolder {
    pub uuid: String,
    pub title: String,
    pub parent_uuid: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct LegacyTag {
    pub uuid: String,
    pub title: String,
    pub color: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct LegacyPart {
    pub uuid: Option<String>,
    pub title: Option<String>,
    pub language: Option<String>,
    pub content: String,
    pub note: String,
}

#[derive(Clone, Debug)]
pub(crate) struct LegacySnippet {
    pub uuid: String,
    pub title: String,
    pub folder_uuid: Option<String>,
    pub tag_uuids: Vec<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub pinned: bool,
    pub locked: bool,
    pub parts: Vec<LegacyPart>,
}
