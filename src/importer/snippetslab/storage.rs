use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::decoder::{Decoded, decoded_string, required_string, unarchive_bytes};
use super::mapping::key;
use super::types::{LegacyFolder, LegacyPart, LegacySnippet, LegacyTag};
use crate::error::{Result, SnipError};

pub(crate) struct LegacyLibrary {
    pub(crate) root: PathBuf,
}

impl LegacyLibrary {
    pub(crate) fn open(path: &Path) -> Result<Self> {
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

    pub(crate) fn decode_file(&self, path: &Path) -> Result<Decoded> {
        let bytes = fs::read(path)?;
        unarchive_bytes(&bytes).map_err(|error| {
            SnipError::validation(format!("cannot decode {}: {error}", path.display()))
        })
    }

    pub(crate) fn identifier(&self) -> Result<String> {
        Ok(self.decode_file(&self.root.join("identifier"))?.text())
    }

    pub(crate) fn version(&self) -> Result<String> {
        let value = self.decode_file(&self.root.join("version.plist"))?;
        Ok(value
            .as_dict()
            .and_then(|dict| dict.get("SnippetsLab"))
            .and_then(Decoded::as_str)
            .unwrap_or("unknown")
            .to_owned())
    }

    pub(crate) fn wrapped_items(&self, name: &str) -> Result<Vec<Decoded>> {
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

    pub(crate) fn folders(&self) -> Result<Vec<LegacyFolder>> {
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

    pub(crate) fn tags(&self) -> Result<Vec<LegacyTag>> {
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

    pub(crate) fn snippets(&self) -> Result<Vec<LegacySnippet>> {
        let mut paths = fs::read_dir(self.root.join("Database/Snippets"))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("data"))
            .collect::<Vec<_>>();
        paths.sort();
        paths.iter().map(|path| self.snippet(path)).collect()
    }

    pub(crate) fn snippet(&self, path: &Path) -> Result<LegacySnippet> {
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
