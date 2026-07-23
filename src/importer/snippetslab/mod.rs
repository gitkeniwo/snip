pub mod decoder;
pub mod mapping;
pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

use crate::domain::{SCHEMA_VERSION, SourceMetadata, TagDefinition, TagRegistry};
use crate::error::{Result, SnipError};
use crate::filesystem::Library;
use crate::service::{CreateOptions, FragmentAddOptions, add_fragment, create_snippet, doctor};

pub use types::ImportReport;

use decoder::parse_uuid;
use mapping::{build_folder_paths, count_files, map_language};
use storage::LegacyLibrary;
use types::{LegacyPart, UNCATEGORIZED_UUID};

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
