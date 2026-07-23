use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::domain::{Fingerprint, SourceMetadata};

#[derive(Clone, Debug, Default)]
pub struct CreateOptions {
    pub id: Option<Uuid>,
    pub fragment_id: Option<Uuid>,
    pub title: String,
    pub folder: Option<String>,
    pub tags: Vec<String>,
    pub language: String,
    pub source_language: Option<String>,
    pub fragment_title: Option<String>,
    pub content: String,
    pub note: Option<String>,
    pub readme: Option<String>,
    pub pinned: bool,
    pub locked: bool,
    pub created_at: Option<String>,
    pub source: Option<SourceMetadata>,
}

#[derive(Clone, Debug, Default)]
pub struct EditOptions {
    pub title: Option<String>,
    pub folder: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
    pub locked: Option<bool>,
    pub fragment_selector: Option<String>,
    pub fragment_title: Option<String>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub note: Option<Option<String>>,
    pub readme: Option<Option<String>>,
    pub if_hash: Option<Fingerprint>,
    pub force: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FragmentAddOptions {
    pub id: Option<Uuid>,
    pub title: String,
    pub language: String,
    pub source_language: Option<String>,
    pub content: String,
    pub note: Option<String>,
    pub if_hash: Option<Fingerprint>,
    pub force: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FragmentEditOptions {
    pub title: Option<String>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub note: Option<Option<String>>,
    pub if_hash: Option<Fingerprint>,
    pub force: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub checked: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub pending_transactions: Vec<String>,
    pub repaired: Vec<String>,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TrashEntry {
    pub entry_id: String,
    pub deleted_at: String,
    pub original_path: String,
    pub snippet_id: Uuid,
    pub title: String,
    pub package_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TrashMetadata {
    pub(crate) schema_version: u32,
    pub(crate) entry_id: String,
    pub(crate) deleted_at: String,
    pub(crate) original_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TransactionState {
    pub(crate) schema_version: u32,
    pub(crate) operation: String,
    pub(crate) original_path: String,
    pub(crate) target_path: String,
}
