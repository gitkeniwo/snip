use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{Result, SnipError};
use crate::filesystem::{Library, atomic_write};

pub(crate) fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(SnipError::validation(format!(
            "symbolic links are not allowed: {}",
            source.display()
        )));
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(SnipError::validation(format!(
                "symbolic links are not allowed: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

pub(crate) fn collect_package_paths(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(path: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(SnipError::validation(format!(
                "symbolic links are not allowed: {}",
                path.display()
            )));
        }
        if path.join("snippet.toml").is_file() {
            result.push(path.to_path_buf());
            return Ok(());
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), result)?;
            }
        }
        Ok(())
    }
    let mut result = Vec::new();
    walk(root, &mut result)?;
    result.sort();
    Ok(result)
}

pub(crate) fn relative_to_root(library: &Library, path: &Path) -> Result<String> {
    path.strip_prefix(library.root())
        .map(path_to_slashes)
        .map_err(|_| SnipError::validation(format!("{} is outside library", path.display())))
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

pub(crate) fn remove_keep_file(path: &Path) {
    let keep = path.join(".keep");
    if keep.exists() {
        let _ = fs::remove_file(keep);
    }
}

pub(crate) fn ensure_keep_for_empty_parents(library: &Library, start: Option<&Path>) {
    let Some(path) = start else {
        return;
    };
    if path == library.snippets_dir() || !path.starts_with(library.snippets_dir()) {
        return;
    }
    if fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none()) {
        let _ = atomic_write(&path.join(".keep"), b"");
    }
}
