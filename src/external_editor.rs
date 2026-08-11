use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::EditorCwdSetting;
use crate::error::{Result, SnipError};
use crate::filesystem::resolve_managed_path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorTargetKind {
    Metadata,
    Readme,
    Content,
    Note,
}

pub fn editor_dir_for_target(
    package_path: &Path,
    fragment_path: Option<&Path>,
    note_relative_path: Option<&str>,
    target: EditorTargetKind,
) -> Option<PathBuf> {
    match target {
        EditorTargetKind::Metadata | EditorTargetKind::Readme => None,
        EditorTargetKind::Content => fragment_path.and_then(Path::parent).map(Path::to_path_buf),
        EditorTargetKind::Note => Some(
            note_relative_path
                .and_then(|relative| resolve_managed_path(package_path, relative).ok())
                .and_then(|note| note.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| package_path.join("notes")),
        ),
    }
}

pub fn resolve_editor_cwd(
    library_root: &Path,
    package_path: &Path,
    leaf_dir: Option<&Path>,
    setting: EditorCwdSetting,
) -> Option<PathBuf> {
    let folder = package_path.parent();
    let fragment = leaf_dir.or(Some(package_path));
    let candidates: &[Option<&Path>] = match setting {
        EditorCwdSetting::Inherit => &[],
        EditorCwdSetting::Library => &[Some(library_root)],
        EditorCwdSetting::Folder => &[folder, Some(library_root)],
        EditorCwdSetting::Snippet => &[Some(package_path), folder, Some(library_root)],
        EditorCwdSetting::Fragment => &[fragment, Some(package_path), folder, Some(library_root)],
    };

    candidates.iter().flatten().find_map(|candidate| {
        candidate
            .is_dir()
            .then(|| candidate.canonicalize().ok())
            .flatten()
    })
}

pub fn launch_editor(
    path: &Path,
    cwd: Option<&Path>,
    configured_editor: Option<&str>,
) -> Result<()> {
    let mut command = editor_command(path, cwd, configured_editor)?;
    let status = command
        .status()
        .map_err(|error| SnipError::io(format!("cannot start editor: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(SnipError::io(format!("editor exited with status {status}")))
    }
}

fn editor_command(
    path: &Path,
    cwd: Option<&Path>,
    configured_editor: Option<&str>,
) -> Result<Command> {
    let editor = configured_editor.map(ToOwned::to_owned).unwrap_or_else(|| {
        std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_owned())
    });
    let parts = shlex::split(&editor)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| SnipError::usage(format!("invalid editor command: {editor:?}")))?;
    let cwd = cwd.filter(|candidate| candidate.is_dir());
    let program = stable_editor_program(&parts[0], cwd.is_some());
    let mut command = Command::new(program);
    command.args(&parts[1..]).arg(path);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    Ok(command)
}

fn stable_editor_program(program: &str, changes_cwd: bool) -> PathBuf {
    let path = Path::new(program);
    if changes_cwd && path.is_relative() && path.components().count() > 1 {
        // Preserve the old best-effort launch behavior if the parent cwd cannot
        // be read; in that rare case the relative program follows the child cwd.
        std::env::current_dir()
            .map(|current| current.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(any(unix, windows))]
    use super::launch_editor;
    use super::{EditorTargetKind, editor_command, editor_dir_for_target, resolve_editor_cwd};
    use crate::config::EditorCwdSetting;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn editor_cwd_values_parse_and_default_to_inherit() {
        assert_eq!(EditorCwdSetting::default(), EditorCwdSetting::Inherit);
        for (value, expected) in [
            ("inherit", EditorCwdSetting::Inherit),
            ("library", EditorCwdSetting::Library),
            ("folder", EditorCwdSetting::Folder),
            ("snippet", EditorCwdSetting::Snippet),
            ("fragment", EditorCwdSetting::Fragment),
        ] {
            assert_eq!(value.parse(), Ok(expected));
        }
        assert!("workspace".parse::<EditorCwdSetting>().is_err());
    }

    #[test]
    fn resolves_each_level_and_falls_back_through_existing_ancestors() {
        let temporary = tempfile::tempdir().unwrap();
        let library = temporary.path().join("Main.sniplib");
        let folder = library.join("snippets/Code");
        let package = folder.join("Example--12345678");
        let fragments = package.join("fragments");
        fs::create_dir_all(&fragments).unwrap();

        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Inherit
            ),
            None
        );
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Library
            ),
            Some(library.canonicalize().unwrap())
        );
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Folder
            ),
            Some(folder.canonicalize().unwrap())
        );
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Snippet
            ),
            Some(package.canonicalize().unwrap())
        );
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Fragment
            ),
            Some(fragments.canonicalize().unwrap())
        );

        fs::remove_dir_all(&fragments).unwrap();
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Fragment
            ),
            Some(package.canonicalize().unwrap())
        );
        fs::remove_dir_all(&package).unwrap();
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Fragment
            ),
            Some(folder.canonicalize().unwrap())
        );
        fs::remove_dir_all(&folder).unwrap();
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Fragment
            ),
            Some(library.canonicalize().unwrap())
        );
        fs::remove_dir_all(&library).unwrap();
        assert_eq!(
            resolve_editor_cwd(
                &library,
                &package,
                Some(&fragments),
                EditorCwdSetting::Fragment
            ),
            None
        );
    }

    #[test]
    fn fragment_without_a_leaf_degrades_to_the_package() {
        let temporary = tempfile::tempdir().unwrap();
        let library = temporary.path().join("Main.sniplib");
        let package = library.join("snippets/Example--12345678");
        fs::create_dir_all(&package).unwrap();

        assert_eq!(
            resolve_editor_cwd(&library, &package, None, EditorCwdSetting::Fragment),
            Some(package.canonicalize().unwrap())
        );
    }

    #[test]
    fn target_directories_cover_all_cli_edit_targets_and_missing_notes() {
        let package = PathBuf::from("/library/snippets/Example--12345678");
        let fragment = package.join("fragments/001-main.rs");

        assert_eq!(
            editor_dir_for_target(&package, Some(&fragment), None, EditorTargetKind::Metadata),
            None
        );
        assert_eq!(
            editor_dir_for_target(&package, Some(&fragment), None, EditorTargetKind::Readme),
            None
        );
        assert_eq!(
            editor_dir_for_target(&package, Some(&fragment), None, EditorTargetKind::Content),
            Some(package.join("fragments"))
        );
        assert_eq!(
            editor_dir_for_target(
                &package,
                Some(&fragment),
                Some("notes/001.md"),
                EditorTargetKind::Note
            ),
            Some(package.join("notes"))
        );
        assert_eq!(
            editor_dir_for_target(&package, None, None, EditorTargetKind::Note),
            Some(package.join("notes"))
        );
    }

    #[test]
    fn launcher_uses_only_a_still_existing_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let cwd = temporary.path().join("cwd");
        fs::create_dir(&cwd).unwrap();
        let target = temporary.path().join("target");

        let command = editor_command(&target, Some(&cwd), Some("editor --wait")).unwrap();
        assert_eq!(command.get_current_dir(), Some(cwd.as_path()));

        let command = editor_command(&target, Some(&cwd), Some("./editor --wait")).unwrap();
        assert_eq!(
            command.get_program(),
            std::env::current_dir()
                .unwrap()
                .join("./editor")
                .as_os_str()
        );

        fs::remove_dir(&cwd).unwrap();
        let command = editor_command(&target, Some(&cwd), Some("editor --wait")).unwrap();
        assert_eq!(command.get_current_dir(), None);
    }

    #[cfg(unix)]
    #[test]
    fn launcher_process_observes_the_resolved_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let cwd = temporary.path().join("cwd");
        fs::create_dir(&cwd).unwrap();
        let capture = temporary.path().join("capture");

        launch_editor(&capture, Some(&cwd), Some("sh -c 'pwd > \"$1\"' sh")).unwrap();

        let observed = fs::read_to_string(capture).unwrap();
        assert_eq!(
            observed.trim(),
            cwd.canonicalize().unwrap().to_str().unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn launcher_process_accepts_a_verbatim_current_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let cwd = temporary.path().join("cwd");
        fs::create_dir(&cwd).unwrap();
        let cwd = cwd.canonicalize().unwrap();
        let capture = temporary.path().join("capture.txt");
        let probe = cwd.join("cwd-probe.cmd");
        fs::write(&probe, "@echo off\r\ncd > \"%~1\"\r\n").unwrap();

        // Keep every configured token static: the POSIX `shlex` parser never
        // sees a dynamic Windows path; `Command` appends `capture` directly.
        launch_editor(&capture, Some(&cwd), Some("cmd /D /C cwd-probe.cmd")).unwrap();

        let observed = fs::read_to_string(&capture).unwrap();
        assert_eq!(PathBuf::from(observed.trim()).canonicalize().unwrap(), cwd);
    }
}
