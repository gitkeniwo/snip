use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::error::{Result, SnipError};

pub fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| SnipError::io(format!("cannot format current time: {error}")))
}

pub fn normalize_tags(tags: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for raw in tags {
        let tag = raw.trim();
        if tag.is_empty() {
            return Err(SnipError::validation("tags cannot be empty"));
        }
        let key = tag.to_lowercase();
        if seen.insert(key) {
            normalized.push(tag.to_owned());
        }
    }
    Ok(normalized)
}

pub fn safe_component(value: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in value.trim().chars() {
        let replace = ch.is_control() || matches!(ch, '/' | '\\' | ':');
        let candidate = if replace {
            (!previous_dash).then_some('-')
        } else {
            Some(ch)
        };
        let Some(candidate) = candidate else {
            continue;
        };
        if result.len() + candidate.len_utf8() > 80 {
            break;
        }
        result.push(candidate);
        previous_dash = candidate == '-';
    }
    let result = result.trim_matches([' ', '.', '-']).to_owned();
    if result.is_empty() || result == "." || result == ".." {
        "untitled".to_owned()
    } else {
        result
    }
}

pub fn package_name(title: &str, id: Uuid) -> String {
    format!(
        "{}--{}",
        safe_component(title),
        &id.simple().to_string()[..8]
    )
}

pub fn fragment_relative_path(index: usize, title: &str, language: &str) -> String {
    let mut name = safe_component(title);
    if !name.contains('.')
        && !is_special_filename(&name)
        && let Some(extension) = extension_for_language(language)
    {
        name.push('.');
        name.push_str(extension);
    }
    format!("fragments/{index:03}-{name}")
}

pub fn note_relative_path(index: usize) -> String {
    format!("notes/{index:03}.md")
}

pub fn extension_for_language(language: &str) -> Option<&'static str> {
    crate::language::info(language).and_then(|info| info.extension)
}

pub fn resolve_managed_path(package: &Path, relative: &str) -> Result<PathBuf> {
    validate_portable_relative_path(relative)?;
    resolve_relative_path(package, relative)
}

pub(crate) fn resolve_relative_path(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SnipError::validation(format!(
            "path must stay inside its root: {relative:?}"
        )));
    }
    Ok(root.join(relative_path))
}

pub(crate) fn validate_portable_relative_path(relative: &str) -> Result<()> {
    validate_relative_path(relative)?;
    if relative.contains(':') {
        return Err(SnipError::validation(format!(
            "path is not portable across supported filesystems: {relative:?}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_relative_path(relative: &str) -> Result<()> {
    let invalid = relative.is_empty()
        || relative.starts_with('/')
        || relative.contains(['\\', '\0'])
        || relative
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."));
    if invalid {
        return Err(SnipError::validation(format!(
            "path must be a safe relative path: {relative:?}"
        )));
    }
    Ok(())
}

pub(crate) fn system_time_rfc3339(value: std::time::SystemTime) -> Option<String> {
    OffsetDateTime::from(value).format(&Rfc3339).ok()
}

pub(crate) fn path_to_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn is_special_filename(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "brewfile" | "dockerfile" | "makefile" | "justfile" | "procfile"
    )
}

#[cfg(test)]
mod tests {
    use super::{resolve_managed_path, safe_component};
    use std::path::Path;

    #[test]
    fn safe_component_never_exceeds_eighty_utf8_bytes() {
        let value = "界".repeat(40);
        let component = safe_component(&value);
        assert_eq!(component.len(), 78);
        assert!(component.len() <= 80);
        assert!(value.starts_with(&component));
    }

    #[test]
    fn managed_paths_use_the_portable_relative_grammar() {
        let package = Path::new("package");
        assert_eq!(
            resolve_managed_path(package, "fragments/001-demo.rs").unwrap(),
            package.join("fragments/001-demo.rs")
        );
        for invalid in [
            "",
            "/absolute",
            "../outside",
            "fragments/./demo",
            "fragments//demo",
            "fragments\\demo",
            "C:/demo",
        ] {
            assert!(
                resolve_managed_path(package, invalid).is_err(),
                "{invalid:?} must be rejected"
            );
        }
    }
}
