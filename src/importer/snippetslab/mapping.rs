use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::types::{KEY_PREFIX, LegacyFolder};
use crate::error::Result;
use crate::filesystem::safe_component;

pub(crate) fn build_folder_paths(folders: &[LegacyFolder]) -> HashMap<String, String> {
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

pub(crate) fn map_language(value: &str) -> &'static str {
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

pub(crate) fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

pub(crate) fn count_files(path: &Path) -> Result<usize> {
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
