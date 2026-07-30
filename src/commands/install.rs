use serde::{Deserialize, Serialize};
use snip::error::{Result, SnipError};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const MANIFEST_VERSION: u32 = 1;

pub struct InstallFile<'a> {
    pub relative_path: PathBuf,
    pub contents: &'a [u8],
}

pub struct InstallReport {
    pub files: Vec<PathBuf>,
    pub changed: usize,
    pub manifest_path: PathBuf,
}

pub struct UninstallReport {
    pub removed: Vec<PathBuf>,
    pub skipped: Vec<PathBuf>,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct InstallManifest {
    version: u32,
    snip_version: String,
    installed_at: String,
    files: BTreeMap<String, String>,
}

pub fn install(
    files: &[InstallFile<'_>],
    root: &Path,
    manifest_relative: &Path,
    force: bool,
    permission_hint: &str,
) -> Result<InstallReport> {
    validate_relative(manifest_relative)?;
    let manifest_path = root.join(manifest_relative);
    let previous = read_manifest(&manifest_path, permission_hint)?;

    for file in files {
        validate_relative(&file.relative_path)?;
        let relative = path_key(&file.relative_path)?;
        let target = root.join(&file.relative_path);
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                let owned = previous
                    .as_ref()
                    .is_some_and(|manifest| manifest.files.contains_key(&relative));
                if metadata.file_type().is_symlink() && !force {
                    return Err(SnipError::conflict(format!(
                        "refusing to replace symbolic link {}; pass --force to replace it",
                        target.display()
                    )));
                }
                if !metadata.file_type().is_file() && !metadata.file_type().is_symlink() {
                    return Err(SnipError::conflict(format!(
                        "installation target is not a regular file: {}",
                        target.display()
                    )));
                }
                if !owned && !force {
                    return Err(SnipError::conflict(format!(
                        "{} already exists and is not recorded in {}; pass --force to replace it",
                        target.display(),
                        manifest_path.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(io_error(
                    "inspect installation target",
                    &target,
                    error,
                    permission_hint,
                ));
            }
        }
    }

    let mut changed = 0;
    let mut manifest_files = BTreeMap::new();
    let mut installed = Vec::with_capacity(files.len());
    for file in files {
        let relative = path_key(&file.relative_path)?;
        let target = root.join(&file.relative_path);
        if fs::symlink_metadata(&target).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            fs::remove_file(&target).map_err(|error| {
                io_error("remove symbolic link", &target, error, permission_hint)
            })?;
        }
        let current = fs::read(&target).ok();
        if current.as_deref() != Some(file.contents) {
            write_file(&target, file.contents, permission_hint)?;
            changed += 1;
        }
        manifest_files.insert(relative, content_hash(file.contents));
        installed.push(target);
    }

    let snip_version = env!("CARGO_PKG_VERSION").to_owned();
    let installed_at = previous
        .as_ref()
        .filter(|manifest| {
            manifest.snip_version == snip_version && manifest.files == manifest_files
        })
        .map(|manifest| manifest.installed_at.clone())
        .unwrap_or(
            OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .map_err(|error| {
                    SnipError::validation(format!("cannot format installation timestamp: {error}"))
                })?,
        );
    let manifest = InstallManifest {
        version: MANIFEST_VERSION,
        snip_version,
        installed_at,
        files: manifest_files,
    };
    let encoded = serde_json::to_vec_pretty(&manifest)?;
    if fs::read(&manifest_path).ok().as_deref() != Some(encoded.as_slice()) {
        write_file(&manifest_path, &encoded, permission_hint)?;
    }

    Ok(InstallReport {
        files: installed,
        changed,
        manifest_path,
    })
}

pub fn uninstall(
    root: &Path,
    manifest_relative: &Path,
    permission_hint: &str,
) -> Result<UninstallReport> {
    validate_relative(manifest_relative)?;
    let manifest_path = root.join(manifest_relative);
    let Some(mut manifest) = read_manifest(&manifest_path, permission_hint)? else {
        return Ok(UninstallReport {
            removed: Vec::new(),
            skipped: Vec::new(),
            manifest_path,
        });
    };

    let mut removed = Vec::new();
    let mut skipped = Vec::new();
    let mut remaining = BTreeMap::new();
    for (relative, expected_hash) in &manifest.files {
        let relative_path = Path::new(relative);
        validate_relative(relative_path)?;
        let target = root.join(relative_path);
        match fs::read(&target) {
            Ok(contents) if content_hash(&contents) == *expected_hash => {
                fs::remove_file(&target).map_err(|error| {
                    io_error("remove installed file", &target, error, permission_hint)
                })?;
                remove_empty_parents(target.parent(), root, permission_hint)?;
                removed.push(target);
            }
            Ok(_) => {
                remaining.insert(relative.clone(), expected_hash.clone());
                skipped.push(target);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                remove_empty_parents(target.parent(), root, permission_hint)?;
            }
            Err(error) => {
                return Err(io_error(
                    "read installed file",
                    &target,
                    error,
                    permission_hint,
                ));
            }
        }
    }

    if remaining.is_empty() {
        fs::remove_file(&manifest_path).map_err(|error| {
            io_error(
                "remove installation manifest",
                &manifest_path,
                error,
                permission_hint,
            )
        })?;
        remove_empty_parents(manifest_path.parent(), root, permission_hint)?;
    } else {
        manifest.files = remaining;
        let encoded = serde_json::to_vec_pretty(&manifest)?;
        write_file(&manifest_path, &encoded, permission_hint)?;
    }

    Ok(UninstallReport {
        removed,
        skipped,
        manifest_path,
    })
}

fn read_manifest(path: &Path, permission_hint: &str) -> Result<Option<InstallManifest>> {
    let contents = match fs::read(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(io_error(
                "read installation manifest",
                path,
                error,
                permission_hint,
            ));
        }
    };
    let manifest: InstallManifest = serde_json::from_slice(&contents).map_err(|error| {
        SnipError::validation(format!(
            "invalid installation manifest {}: {error}",
            path.display()
        ))
    })?;
    if manifest.version != MANIFEST_VERSION {
        return Err(SnipError::validation(format!(
            "unsupported installation manifest version {} in {}",
            manifest.version,
            path.display()
        )));
    }
    for relative in manifest.files.keys() {
        validate_relative(Path::new(relative))?;
    }
    Ok(Some(manifest))
}

fn write_file(path: &Path, contents: &[u8], permission_hint: &str) -> Result<()> {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(SnipError::conflict(format!(
            "refusing to write through symbolic link {}",
            path.display()
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| SnipError::validation(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|error| io_error("create directory", parent, error, permission_hint))?;
    fs::write(path, contents)
        .map_err(|error| io_error("write file", path, error, permission_hint))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644))
            .map_err(|error| io_error("set file permissions", path, error, permission_hint))?;
    }
    Ok(())
}

fn remove_empty_parents(start: Option<&Path>, root: &Path, permission_hint: &str) -> Result<()> {
    let Some(mut directory) = start.map(Path::to_path_buf) else {
        return Ok(());
    };
    while directory != root && directory.starts_with(root) {
        match fs::remove_dir(&directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                ) =>
            {
                break;
            }
            Err(error) => {
                return Err(io_error(
                    "remove empty directory",
                    &directory,
                    error,
                    permission_hint,
                ));
            }
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent.to_path_buf();
    }
    Ok(())
}

fn validate_relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(SnipError::validation(format!(
            "installation path must be relative and cannot contain traversal: {}",
            path.display()
        )));
    }
    Ok(())
}

fn path_key(path: &Path) -> Result<String> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| {
            SnipError::validation(format!(
                "installation path is not valid UTF-8: {}",
                path.display()
            ))
        })
}

fn content_hash(contents: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(contents).to_hex())
}

fn io_error(action: &str, path: &Path, error: io::Error, permission_hint: &str) -> SnipError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        SnipError::io(format!(
            "permission denied while trying to {action} {}: {error}; {permission_hint}",
            path.display()
        ))
    } else {
        SnipError::io(format!("cannot {action} {}: {error}", path.display()))
    }
}
