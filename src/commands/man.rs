use serde_json::json;
use snip::config::AppConfig;
use snip::error::{Result, SnipError};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use super::install::{InstallFile, install, uninstall};
use super::man_pages::PAGES;
use super::output::{print_record, resolve_output};
use crate::cli::{ManArgs, ManCommand, OutputMode};

const MANIFEST_RELATIVE: &str = "snip/man-install.json";

struct Layout {
    root: PathBuf,
    man_relative: PathBuf,
    manifest_relative: PathBuf,
}

struct ManagedInstallation {
    manager: &'static str,
    uninstall_hint: &'static str,
}

pub fn command_man(args: &ManArgs, explicit_output: Option<OutputMode>) -> Result<()> {
    if let ManCommand::Show { page } = &args.command {
        return command_show(page.as_deref().unwrap_or("snip"));
    }
    let output = match explicit_output {
        Some(output) => output,
        None => resolve_output(None, &AppConfig::load()?),
    };
    match &args.command {
        ManCommand::Path { prefix } => command_path(prefix.as_deref(), output),
        ManCommand::Install { prefix, force } => command_install(prefix.as_deref(), *force, output),
        ManCommand::Uninstall { prefix } => command_uninstall(prefix.as_deref(), output),
        ManCommand::Generate { directory } => command_generate(directory, output),
        ManCommand::Show { .. } => unreachable!("handled before resolving output"),
    }
}

fn command_path(prefix: Option<&Path>, output: OutputMode) -> Result<()> {
    let layout = layout(prefix)?;
    let path = layout.root.join(layout.man_relative);
    if output == OutputMode::Human {
        println!("{}", path.display());
        Ok(())
    } else {
        print_record(&json!({"path": path}), output)
    }
}

fn command_install(prefix: Option<&Path>, force: bool, output: OutputMode) -> Result<()> {
    unsupported_on_windows("install")?;
    if !force && let Some(managed) = managed_installation()? {
        eprintln!(
            "warning: this snip binary appears to be managed by {}; install its man pages through the same package manager",
            managed.manager
        );
        return Err(SnipError::conflict(
            "refusing to install separate man pages for a package-managed binary; pass --force to override",
        ));
    }

    let layout = layout(prefix)?;
    let files = installation_files(&layout.man_relative)?;
    let permission_hint = "retry with `sudo snip man install --prefix /usr/local` if a system-wide install is intended";
    let report = install(
        &files,
        &layout.root,
        &layout.manifest_relative,
        force,
        permission_hint,
    )?;
    let directory = layout.root.join(&layout.man_relative);
    if output == OutputMode::Human {
        println!(
            "installed {} man pages in {} ({} updated, {} obsolete removed)",
            report.files.len(),
            directory.display(),
            report.changed,
            report.pruned.len()
        );
        println!("manifest: {}", report.manifest_path.display());
    } else {
        print_record(
            &json!({
                "directory": directory,
                "manifest": report.manifest_path,
                "files": report.files,
                "updated": report.changed,
                "pruned": report.pruned,
            }),
            output,
        )?;
    }
    warn_if_manpath_missing(&directory);
    Ok(())
}

fn command_uninstall(prefix: Option<&Path>, output: OutputMode) -> Result<()> {
    unsupported_on_windows("uninstall")?;
    if let Some(managed) = managed_installation()? {
        eprintln!(
            "warning: this snip binary appears to be managed by {}",
            managed.manager
        );
        return Err(SnipError::conflict(format!(
            "remove snip and its man pages with {}",
            managed.uninstall_hint
        )));
    }

    let layout = layout(prefix)?;
    let permission_hint = "retry with `sudo snip man uninstall --prefix /usr/local` if this was a system-wide install";
    let report = uninstall(&layout.root, &layout.manifest_relative, permission_hint)?;
    if output == OutputMode::Human {
        for path in &report.skipped {
            println!("skipped (modified): {}", path.display());
        }
        println!("removed {} man pages", report.removed.len());
        if report.removed.is_empty() && report.skipped.is_empty() {
            println!(
                "no installation manifest found: {}",
                report.manifest_path.display()
            );
        }
    } else {
        print_record(
            &json!({
                "manifest": report.manifest_path,
                "removed": report.removed,
                "skipped_modified": report.skipped,
            }),
            output,
        )?;
    }
    Ok(())
}

fn command_show(page: &str) -> Result<()> {
    unsupported_on_windows("show")?;
    let (filename, contents) = find_page(page)?;
    let mut temporary = tempfile::Builder::new()
        .prefix("snip-man-")
        .suffix(&format!("-{filename}"))
        .tempfile()
        .map_err(|error| SnipError::io(format!("cannot create temporary man page: {error}")))?;
    temporary.write_all(contents)?;
    temporary.flush()?;
    let path = temporary.path().canonicalize()?;
    let status = ProcessCommand::new("man")
        .arg(&path)
        .status()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                SnipError::not_found(format!(
                    "could not launch `man`; embedded page was written to {}",
                    path.display()
                ))
            } else {
                SnipError::io(format!("cannot launch `man`: {error}"))
            }
        })?;
    if !status.success() {
        return Err(SnipError::io(format!("`man` exited with status {status}")));
    }
    Ok(())
}

fn command_generate(directory: &Path, output: OutputMode) -> Result<()> {
    fs::create_dir_all(directory).map_err(|error| {
        SnipError::io(format!(
            "cannot create export directory {}: {error}",
            directory.display()
        ))
    })?;
    for (name, _) in PAGES {
        let path = export_path(directory, name)?;
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(SnipError::conflict(format!(
                "refusing to write through symbolic link {}",
                path.display()
            )));
        }
    }
    let mut paths = Vec::with_capacity(PAGES.len());
    for (name, contents) in PAGES {
        let path = export_path(directory, name)?;
        let parent = path.parent().ok_or_else(|| {
            SnipError::validation(format!(
                "manual page path has no parent: {}",
                path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            SnipError::io(format!(
                "cannot create manual page directory {}: {error}",
                parent.display()
            ))
        })?;
        fs::write(&path, contents).map_err(|error| {
            SnipError::io(format!("cannot write man page {}: {error}", path.display()))
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).map_err(|error| {
                SnipError::io(format!(
                    "cannot set man page permissions {}: {error}",
                    path.display()
                ))
            })?;
        }
        paths.push(path);
    }
    if output == OutputMode::Human {
        println!(
            "generated {} man pages in {}",
            paths.len(),
            directory.display()
        );
        Ok(())
    } else {
        print_record(&json!({"directory": directory, "files": paths}), output)
    }
}

fn layout(prefix: Option<&Path>) -> Result<Layout> {
    if let Some(prefix) = prefix {
        return Ok(Layout {
            root: prefix.to_path_buf(),
            man_relative: PathBuf::from("share/man"),
            manifest_relative: PathBuf::from("share").join(MANIFEST_RELATIVE),
        });
    }

    if let Some(data_home) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return Ok(Layout {
            root: PathBuf::from(data_home),
            man_relative: PathBuf::from("man"),
            manifest_relative: PathBuf::from(MANIFEST_RELATIVE),
        });
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            SnipError::io("cannot resolve user man page directory: HOME or USERPROFILE is not set")
        })?;
    Ok(Layout {
        root: PathBuf::from(home).join(".local"),
        man_relative: PathBuf::from("share/man"),
        manifest_relative: PathBuf::from("share").join(MANIFEST_RELATIVE),
    })
}

fn installation_files(man_relative: &Path) -> Result<Vec<InstallFile<'static>>> {
    PAGES
        .iter()
        .map(|(name, contents)| {
            let section = page_section(name)?;
            Ok(InstallFile {
                relative_path: man_relative.join(format!("man{section}")).join(name),
                contents,
            })
        })
        .collect()
}

fn export_path(directory: &Path, name: &str) -> Result<PathBuf> {
    let section = page_section(name)?;
    Ok(directory.join(format!("man{section}")).join(name))
}

fn page_section(name: &str) -> Result<&str> {
    let (_, section) = name.rsplit_once('.').ok_or_else(|| {
        SnipError::validation(format!("embedded manual page has no section: {name}"))
    })?;
    if matches!(section, "1" | "5" | "7") {
        Ok(section)
    } else {
        Err(SnipError::validation(format!(
            "unsupported embedded manual section in {name}"
        )))
    }
}

fn find_page(page: &str) -> Result<(&'static str, &'static [u8])> {
    if page.is_empty()
        || !page
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return Err(SnipError::usage(format!("invalid man page name `{page}`")));
    }
    let explicit_section = match page.rsplit_once('.') {
        Some((stem, section)) if !stem.contains('.') && matches!(section, "1" | "5" | "7") => {
            Some((stem, section))
        }
        Some(_) => {
            return Err(SnipError::usage(format!(
                "invalid man page name `{page}`; supported sections are 1, 5, and 7"
            )));
        }
        None => None,
    };
    let sections = explicit_section
        .map(|(_, section)| vec![section])
        .unwrap_or_else(|| vec!["1", "5", "7"]);
    let stem = explicit_section.map_or(page, |(stem, _)| stem);
    let mut candidates = sections
        .iter()
        .map(|section| format!("{stem}.{section}"))
        .collect::<Vec<_>>();
    if !stem.starts_with("snip-") {
        candidates.extend(
            sections
                .iter()
                .map(|section| format!("snip-{stem}.{section}")),
        );
    }
    let matching_page = candidates
        .iter()
        .find_map(|filename| PAGES.iter().find(|(name, _)| *name == filename).copied());
    matching_page.ok_or_else(|| {
        SnipError::not_found(format!(
            "unknown man page `{page}`; try `snip` or a generated command page"
        ))
    })
}

fn managed_installation() -> Result<Option<ManagedInstallation>> {
    let executable = env::current_exe()
        .map_err(|error| SnipError::io(format!("cannot locate current executable: {error}")))?
        .canonicalize()
        .map_err(|error| SnipError::io(format!("cannot resolve current executable: {error}")))?;
    Ok(managed_installation_for(&executable))
}

fn managed_installation_for(executable: &Path) -> Option<ManagedInstallation> {
    let path = executable.to_string_lossy().replace('\\', "/");
    if path.contains("/Cellar/") || path.starts_with("/home/linuxbrew/.linuxbrew/") {
        Some(ManagedInstallation {
            manager: "Homebrew",
            uninstall_hint: "`brew uninstall snip`",
        })
    } else if path.starts_with("/usr/bin/") {
        Some(ManagedInstallation {
            manager: "the system package manager",
            uninstall_hint: "the system package manager that installed snip",
        })
    } else if path.starts_with("/nix/store/") {
        Some(ManagedInstallation {
            manager: "Nix",
            uninstall_hint: "Nix",
        })
    } else {
        None
    }
}

fn warn_if_manpath_missing(man_root: &Path) {
    let man_root = man_root
        .canonicalize()
        .unwrap_or_else(|_| man_root.to_path_buf());
    let discovered = ProcessCommand::new("manpath")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .or_else(|| {
            ProcessCommand::new("man")
                .arg("-w")
                .output()
                .ok()
                .filter(|output| output.status.success())
        });
    let included = discovered.as_ref().is_some_and(|output| {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .split(':')
            .map(PathBuf::from)
            .map(|path| path.canonicalize().unwrap_or(path))
            .any(|path| path == man_root)
    });
    if !included {
        eprintln!(
            "warning: {} was not found in MANPATH; add `export MANPATH=\"{}:${{MANPATH:-}}\"` to your shell profile",
            man_root.display(),
            man_root.display()
        );
    }
}

#[cfg(windows)]
fn unsupported_on_windows(operation: &str) -> Result<()> {
    Err(SnipError::usage(format!(
        "`snip man {operation}` is not supported on Windows"
    )))
}

#[cfg(not(windows))]
fn unsupported_on_windows(_operation: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{find_page, managed_installation_for};
    use std::path::Path;

    #[test]
    fn package_manager_paths_are_classified_without_blocking_user_installs() {
        assert_eq!(
            managed_installation_for(Path::new("/opt/homebrew/Cellar/snip/0.2.1/bin/snip"))
                .unwrap()
                .manager,
            "Homebrew"
        );
        assert_eq!(
            managed_installation_for(Path::new("/home/linuxbrew/.linuxbrew/bin/snip"))
                .unwrap()
                .manager,
            "Homebrew"
        );
        assert_eq!(
            managed_installation_for(Path::new("/usr/bin/snip"))
                .unwrap()
                .manager,
            "the system package manager"
        );
        assert_eq!(
            managed_installation_for(Path::new("/nix/store/0123456789-snip-0.2.1/bin/snip"))
                .unwrap()
                .manager,
            "Nix"
        );
        assert!(managed_installation_for(Path::new("/home/me/.cargo/bin/snip")).is_none());
        assert!(managed_installation_for(Path::new("/home/me/.local/bin/snip")).is_none());
    }

    #[test]
    fn short_man_page_names_fall_back_to_the_snip_prefix() {
        assert_eq!(find_page("create").unwrap().0, "snip-create.1");
        assert_eq!(find_page("man").unwrap().0, "snip-man.1");
        assert_eq!(find_page("snip-create").unwrap().0, "snip-create.1");
        assert_eq!(find_page("sniplib").unwrap().0, "sniplib.5");
        assert_eq!(find_page("sniplib.5").unwrap().0, "sniplib.5");
        assert_eq!(find_page("config").unwrap().0, "snip-config.1");
        assert_eq!(find_page("config.5").unwrap().0, "snip-config.5");
        assert_eq!(find_page("agents").unwrap().0, "snip-agents.7");
        assert!(find_page("snip.9").is_err());
        assert!(find_page("does-not-exist").is_err());
    }
}
