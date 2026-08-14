use std::collections::HashSet;
use std::fs;

use super::helpers::collect_package_paths;
use super::types::{DoctorReport, TransactionState};
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
            for path in paths {
                checked += 1;
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
        }
        Err(error) => errors.push(error.to_string()),
    }
    let active_pending = if repair_lock.is_some() {
        list_transaction_names(library)
    } else {
        pending_transactions
    };
    let ok = errors.is_empty() && active_pending.is_empty();
    DoctorReport {
        checked,
        errors,
        warnings,
        pending_transactions: active_pending,
        repaired,
        ok,
    }
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
