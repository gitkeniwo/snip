use std::collections::HashSet;
use std::fs;

use super::helpers::{collect_package_paths, path_to_slashes};
use super::portability::{collisions, describe_collision, non_portable_paths};
use super::trash::validate_trash_metadata;
use super::types::{DoctorReport, TransactionState, TrashMetadata};
use crate::domain::ChangeSet;
use crate::error::{Result, SnipError};
use crate::filesystem::io::{
    reject_symlink, validate_rfc3339, validate_schema, validate_snippet_metadata,
};
use crate::filesystem::paths::{resolve_relative_path, validate_portable_relative_path};
use crate::filesystem::{Library, package_name};

pub fn organize(library: &Library, dry_run: bool) -> Result<Vec<ChangeSet>> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let mut changes = Vec::new();
    for snippet in catalog.snippets {
        let target = snippet
            .package_path
            .parent()
            .unwrap_or_else(|| library.root())
            .join(package_name(&snippet.title, snippet.id));
        if target == snippet.package_path {
            continue;
        }
        if target.exists() {
            return Err(SnipError::conflict(format!(
                "organize target already exists: {}",
                target.display()
            )));
        }
        if !dry_run {
            fs::rename(&snippet.package_path, &target)?;
        }
        changes.push(ChangeSet {
            fields: vec!["package_path".to_owned()],
            old_fingerprint: Some(snippet.fingerprint.clone()),
            new_fingerprint: Some(snippet.fingerprint),
            old_path: Some(snippet.package_path),
            new_path: Some(target),
        });
    }
    Ok(changes)
}

pub fn doctor(library: &Library, repair: bool) -> DoctorReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut repaired = Vec::new();
    let mut checked = 0;
    let mut warned_folders = HashSet::new();
    for name in library.restored() {
        repaired.push(format!("recreated missing directory: {name}"));
    }
    let library_manifest_path = library.root().join("snip.toml");
    if library.manifest().name.trim().is_empty() {
        errors.push(format!(
            "{} has an empty library name",
            library_manifest_path.display()
        ));
    }
    if let Err(error) = validate_rfc3339(
        &library.manifest().created_at,
        "created_at",
        &library_manifest_path,
    ) {
        errors.push(error.to_string());
    }
    let repair_lock = if repair {
        match library.lock() {
            Ok(lock) => Some(lock),
            Err(error) => {
                errors.push(format!("cannot repair without the library lock: {error}"));
                None
            }
        }
    } else {
        None
    };
    let pending_transactions = list_transaction_names(library);
    if let Err(error) = library.tag_registry() {
        errors.push(error.to_string());
    }
    if repair_lock.is_some() {
        for name in &pending_transactions {
            match recover_transaction(library, name) {
                Ok(message) => repaired.push(message),
                Err(error) => errors.push(format!("transaction {name}: {error}")),
            }
        }
    }
    match collect_package_paths(&library.snippets_dir()) {
        Ok(paths) => {
            let mut ids = HashSet::new();
            // Slash paths of every package, relative to snippets/. Collected
            // from disk rather than from `snippet.folder` so that a package
            // whose metadata fails to load still contributes its directories,
            // and so the package name itself takes part in sibling checks.
            let mut tree = Vec::with_capacity(paths.len());
            for path in paths {
                checked += 1;
                if let Ok(relative) = path.strip_prefix(library.snippets_dir()) {
                    tree.push(path_to_slashes(relative));
                }
                match library.load_snippet(&path) {
                    Ok(snippet) => {
                        let manifest_path = path.join("snippet.toml");
                        if let Err(error) =
                            validate_snippet_metadata(&snippet.manifest, &manifest_path)
                        {
                            errors.push(error.to_string());
                        }
                        if !ids.insert(snippet.id) {
                            errors.push(format!("duplicate snippet UUID: {}", snippet.id));
                        }
                        let expected = package_name(&snippet.title, snippet.id);
                        if path.file_name().and_then(|value| value.to_str()) != Some(&expected) {
                            warnings.push(format!(
                                "{}: package name differs from canonical {expected:?}",
                                path.display()
                            ));
                        }
                        if !snippet.folder.is_empty()
                            && validate_portable_relative_path(&snippet.folder).is_err()
                            && warned_folders.insert(snippet.folder.clone())
                        {
                            warnings.push(format!(
                                "{}: folder path is not portable across supported filesystems",
                                library.snippets_dir().join(&snippet.folder).display()
                            ));
                        }
                    }
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
            // Portability recommendations from FORMAT.md, "Folders": schema v1
            // does not make these validity requirements, so they are warnings
            // and an affected library stays fully usable on this machine.
            for collision in collisions(&tree) {
                warnings.push(describe_collision(&library.snippets_dir(), &collision));
            }
            for (relative, reason) in non_portable_paths(&tree) {
                warnings.push(format!(
                    "{}: path component {reason}",
                    library.snippets_dir().join(&relative).display()
                ));
            }
        }
        Err(error) => errors.push(error.to_string()),
    }
    let checked_trash = scan_trash(library, &mut errors, &mut warnings);
    let active_pending = if repair_lock.is_some() {
        list_transaction_names(library)
    } else {
        pending_transactions
    };
    let ok = errors.is_empty() && active_pending.is_empty();
    DoctorReport {
        checked,
        checked_trash,
        errors,
        warnings,
        pending_transactions: active_pending,
        repaired,
        ok,
    }
}

/// Validates `trash/` the way the package walk validates `snippets/`, and
/// returns the number of entries examined.
///
/// `trash_entries` cannot be reused here: it stops at the first bad entry and
/// silently skips directories that lack `trash.toml` or `package/`, which is
/// exactly how a malformed entry stays invisible until someone tries to
/// restore it. Each entry is therefore reported on its own, and one broken
/// entry does not hide the rest.
fn scan_trash(library: &Library, errors: &mut Vec<String>, warnings: &mut Vec<String>) -> usize {
    let mut checked = 0;
    let read = match fs::read_dir(library.trash_dir()) {
        Ok(read) => read,
        Err(error) => {
            errors.push(format!(
                "{}: cannot read trash: {error}",
                library.trash_dir().display()
            ));
            return 0;
        }
    };
    let mut entries = read.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if let Err(error) = reject_symlink(&path) {
            errors.push(format!("{}: {error}", path.display()));
            continue;
        }
        if !path.is_dir() {
            warnings.push(format!(
                "{}: unexpected file in trash; snip only writes entry directories there",
                path.display()
            ));
            continue;
        }
        let metadata_path = path.join("trash.toml");
        let package_path = path.join("package");
        let has_metadata = metadata_path.is_file();
        let has_package = package_path.is_dir();
        match (has_metadata, has_package) {
            (false, false) => {
                warnings.push(format!(
                    "{}: unrecognized directory in trash; it has neither trash.toml nor package/",
                    path.display()
                ));
                continue;
            }
            (false, true) => {
                // The payload is intact but unreachable: `snip trash` skips the
                // entry, so the snippet can be neither listed nor restored.
                errors.push(format!(
                    "{}: trash entry has no trash.toml, so the package is not listed and cannot be restored",
                    path.display()
                ));
                continue;
            }
            (true, false) => {
                errors.push(format!(
                    "{}: trash entry has no package/ directory",
                    path.display()
                ));
                continue;
            }
            (true, true) => {}
        }
        checked += 1;
        if let Err(error) = check_trash_entry(library, &metadata_path, &package_path) {
            errors.push(format!("{}: {error}", path.display()));
        }
    }
    checked
}

fn check_trash_entry(
    library: &Library,
    metadata_path: &std::path::Path,
    package_path: &std::path::Path,
) -> Result<()> {
    reject_symlink(metadata_path)?;
    let metadata: TrashMetadata = toml::from_str(&fs::read_to_string(metadata_path)?)?;
    validate_trash_metadata(library, &metadata, metadata_path)?;
    let snippet = library.load_snippet(package_path)?;
    // `load_snippet` enforces structure, paths, and UTF-8 but leaves semantic
    // metadata to doctor, exactly as it does for a live package. Without this
    // the entry would pass here and fail on restore.
    validate_snippet_metadata(&snippet.manifest, &package_path.join("snippet.toml"))?;
    Ok(())
}

fn list_transaction_names(library: &Library) -> Vec<String> {
    fs::read_dir(library.transactions_dir())
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn recover_transaction(library: &Library, name: &str) -> Result<String> {
    let directory = library.transactions_dir().join(name);
    let state_path = directory.join("transaction.toml");
    if !state_path.is_file() {
        let backup = directory.join("backup");
        if backup.exists() {
            return Err(SnipError::validation(format!(
                "missing transaction.toml; backup at {} was left in place",
                backup.display()
            )));
        }
        fs::remove_dir_all(&directory)?;
        return Ok(format!("removed incomplete transaction {name}"));
    }
    reject_symlink(&state_path)?;
    let state: TransactionState = toml::from_str(&fs::read_to_string(&state_path)?)?;
    validate_schema(state.schema_version, &state_path)?;
    if state.operation != "replace" {
        return Err(SnipError::validation(format!(
            "unsupported transaction operation {:?}",
            state.operation
        )));
    }
    let original = resolve_transaction_path(library, &state.original_path)?;
    let target = resolve_transaction_path(library, &state.target_path)?;
    let backup = directory.join("backup");
    let staged = directory.join("staged");
    if target.exists() {
        if let Err(target_error) = library.load_snippet(&target) {
            if !backup.exists() {
                return Err(SnipError::validation(format!(
                    "transaction target is invalid and no backup exists: {target_error}"
                )));
            }
            library.load_snippet(&backup).map_err(|backup_error| {
                SnipError::validation(format!(
                    "transaction target is invalid ({target_error}) and backup at {} is invalid ({backup_error})",
                    backup.display()
                ))
            })?;
            fs::remove_dir_all(&target)?;
            if let Some(parent) = original.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&backup, &original)?;
            if staged.exists() {
                fs::remove_dir_all(&staged)?;
            }
            fs::remove_dir_all(&directory)?;
            return Ok(format!("rolled back transaction {name}"));
        }
        if backup.exists() {
            fs::remove_dir_all(&backup)?;
        }
        if staged.exists() {
            fs::remove_dir_all(&staged)?;
        }
        fs::remove_dir_all(&directory)?;
        return Ok(format!("completed transaction {name}"));
    }
    if backup.exists() {
        library.load_snippet(&backup).map_err(|backup_error| {
            SnipError::validation(format!(
                "transaction backup at {} is invalid and was left in place: {backup_error}",
                backup.display()
            ))
        })?;
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&backup, &original)?;
    }
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::remove_dir_all(&directory)?;
    Ok(format!("rolled back transaction {name}"))
}

fn resolve_transaction_path(library: &Library, relative: &str) -> Result<std::path::PathBuf> {
    let path = resolve_relative_path(library.root(), relative)?;
    if path == library.snippets_dir() || !path.starts_with(library.snippets_dir()) {
        return Err(SnipError::validation(format!(
            "transaction path must name a package below snippets/: {relative:?}"
        )));
    }
    Ok(path)
}
