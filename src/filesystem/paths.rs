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
        if replace {
            if !previous_dash {
                result.push('-');
                previous_dash = true;
            }
        } else {
            result.push(ch);
            previous_dash = ch == '-';
        }
        if result.len() >= 80 {
            while !result.is_char_boundary(result.len()) {
                result.pop();
            }
            break;
        }
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
    match language.to_ascii_lowercase().as_str() {
        "bash" | "shell" | "sh" => Some("sh"),
        "fish" => Some("fish"),
        "python" => Some("py"),
        "rust" => Some("rs"),
        "javascript" | "js" => Some("js"),
        "typescript" | "ts" => Some("ts"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "markdown" | "md" => Some("md"),
        "html" => Some("html"),
        "css" => Some("css"),
        "sql" => Some("sql"),
        "go" | "golang" => Some("go"),
        "tcl" => Some("tcl"),
        "dockerfile" | "makefile" | "text" | "plain" => None,
        _ => None,
    }
}

pub fn resolve_managed_path(package: &Path, relative: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(SnipError::validation(format!(
            "managed path must stay inside the snippet package: {relative:?}"
        )));
    }
    Ok(package.join(relative_path))
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
