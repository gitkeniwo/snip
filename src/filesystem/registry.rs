use std::collections::HashSet;
use std::fs;
use uuid::Uuid;

use crate::domain::{SCHEMA_VERSION, TagDefinition, TagRegistry};
use crate::error::{Result, SnipError};

use super::io::{atomic_write, validate_schema};
use super::library::Library;
use super::paths::normalize_tags;

impl Library {
    pub fn tag_registry(&self) -> Result<TagRegistry> {
        let path = self.root.join("tags.toml");
        if !path.exists() {
            return Ok(TagRegistry {
                schema_version: SCHEMA_VERSION,
                tags: Vec::new(),
                extra: toml::Table::new(),
            });
        }
        let registry: TagRegistry =
            toml::from_str(&fs::read_to_string(&path).map_err(|error| {
                SnipError::validation(format!("cannot read {}: {error}", path.display()))
            })?)
            .map_err(|error| {
                SnipError::validation(format!("cannot parse {}: {error}", path.display()))
            })?;
        validate_schema(registry.schema_version, &path)?;
        let mut seen_names = HashSet::new();
        let mut seen_ids = HashSet::new();
        for tag in &registry.tags {
            if tag.name.trim().is_empty() {
                return Err(SnipError::validation(format!(
                    "{} contains an empty tag name",
                    path.display()
                )));
            }
            if !seen_names.insert(tag.name.trim().to_lowercase()) {
                return Err(SnipError::validation(format!(
                    "{} contains duplicate tag name {:?}",
                    path.display(),
                    tag.name
                )));
            }
            if !seen_ids.insert(tag.id) {
                return Err(SnipError::validation(format!(
                    "{} contains duplicate tag UUID {}",
                    path.display(),
                    tag.id
                )));
            }
        }
        Ok(registry)
    }

    pub fn write_tag_registry(&self, registry: &TagRegistry) -> Result<()> {
        let data = toml::to_string_pretty(registry)?;
        atomic_write(&self.root.join("tags.toml"), data.as_bytes())
    }

    pub fn register_tags(&self, names: &[String]) -> Result<()> {
        let normalized = normalize_tags(names)?;
        let mut registry = self.tag_registry()?;
        let known = registry
            .tags
            .iter()
            .map(|tag| tag.name.to_lowercase())
            .collect::<HashSet<_>>();
        let mut changed = false;
        for name in normalized {
            if !known.contains(&name.to_lowercase()) {
                registry.tags.push(TagDefinition {
                    id: Uuid::new_v4(),
                    name,
                    color: None,
                    source_id: None,
                    extra: toml::Table::new(),
                });
                changed = true;
            }
        }
        if changed {
            registry.tags.sort_by_key(|left| left.name.to_lowercase());
            self.write_tag_registry(&registry)?;
        }
        Ok(())
    }
}
