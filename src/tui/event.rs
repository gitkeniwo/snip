use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify_debouncer_mini::{Config, DebounceEventResult, Debouncer, new_debouncer_opt};

use crate::error::{Result, SnipError};
use crate::git::ActionOutcome;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEvent {
    FsChanged,
    GitFinished(GitTaskResult),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitTaskResult {
    pub action: &'static str,
    pub outcome: std::result::Result<ActionOutcome, String>,
    /// Whether this was triggered by a manual user action (as opposed to an
    /// automatic background task). Manual results show a status toast and clear
    /// the `operation_queued` flag.
    pub manual: bool,
}

pub struct WatchHandle {
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

pub fn start_watcher(root: &Path, sender: Sender<AppEvent>) -> Result<WatchHandle> {
    let debouncer =
        start_watcher_with::<notify::RecommendedWatcher>(root, notify::Config::default(), sender)?;
    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}

fn start_watcher_with<T: Watcher>(
    root: &Path,
    notify_config: notify::Config,
    sender: Sender<AppEvent>,
) -> Result<Debouncer<T>> {
    let root = root.to_path_buf();
    let callback_root = root.clone();
    let config = Config::default()
        .with_timeout(Duration::from_millis(250))
        .with_batch_mode(false)
        .with_notify_config(notify_config);
    let mut debouncer = new_debouncer_opt::<_, T>(config, move |result: DebounceEventResult| {
        let Ok(events) = result else {
            return;
        };
        if events
            .iter()
            .any(|event| is_relevant(&callback_root, &event.path))
        {
            let _ = sender.send(AppEvent::FsChanged);
        }
    })
    .map_err(|error| SnipError::io(format!("cannot start filesystem watcher: {error}")))?;
    debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|error| SnipError::io(format!("cannot watch {}: {error}", root.display())))?;
    Ok(debouncer)
}

fn is_relevant(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    !relative.components().any(|component| {
        let value = component.as_os_str();
        value == ".snip" || value == ".git"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    use crate::config::{AppConfig, GitConfig};
    use crate::filesystem::Library;
    use crate::tui::app::App;

    #[test]
    fn internal_cache_and_git_events_are_ignored() {
        let root = Path::new("/library");
        assert!(is_relevant(root, Path::new("/library/snippets/a/file")));
        assert!(!is_relevant(
            root,
            Path::new("/library/.snip/locks/library.lock")
        ));
        assert!(!is_relevant(root, Path::new("/library/.git/index")));
    }

    #[test]
    fn debounced_watcher_reports_managed_file_changes() {
        let temporary = tempfile::tempdir().unwrap();
        let notify_config = notify::Config::default().with_poll_interval(Duration::from_millis(50));
        let (sender, receiver) = mpsc::channel();
        let _watcher =
            start_watcher_with::<notify::PollWatcher>(temporary.path(), notify_config, sender)
                .unwrap();
        std::fs::write(temporary.path().join("changed.txt"), "changed").unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            AppEvent::FsChanged
        );
    }

    #[test]
    fn watcher_handles_a_large_library_tree() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        for index in 0..400 {
            std::fs::create_dir_all(root.join(format!("snippets/F{index}/S{index}/notes")))
                .unwrap();
        }

        // This specifically guards against regressions where recursive watching opens
        // one file descriptor per managed package and fails on ordinary libraries.
        let (sender, _receiver) = mpsc::channel();
        let _watcher = start_watcher(root, sender).unwrap();
    }

    #[test]
    fn automatic_commit_does_not_emit_a_filesystem_change() {
        if !Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return;
        }
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("Automatic.sniplib");
        let library = Library::init(&root, Some("Automatic")).unwrap();
        crate::git::init(&root).unwrap();
        for (key, value) in [
            ("user.name", "snip CI"),
            ("user.email", "ci@example.invalid"),
        ] {
            assert!(
                Command::new("git")
                    .args(["-C", root.to_str().unwrap(), "config", key, value])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let repo = crate::git::probe(&root).unwrap();
        crate::git::commit(&repo, "initial").unwrap();
        let config = AppConfig {
            git: Some(GitConfig {
                auto_commit_interval: 1,
                ..GitConfig::default()
            }),
            ..AppConfig::default()
        };
        let mut app = App::new(library, &config).unwrap();
        let tags_path = root.join("tags.toml");
        let mut tags = std::fs::read_to_string(&tags_path).unwrap();
        tags.push_str("\n# automatic\n");
        std::fs::write(tags_path, tags).unwrap();
        app.refresh_git();
        app.git
            .status
            .as_mut()
            .unwrap()
            .last_commit
            .as_mut()
            .unwrap()
            .timestamp -= 120;

        let notify_config = notify::Config::default().with_poll_interval(Duration::from_millis(50));
        let (sender, receiver) = mpsc::channel();
        let _watcher =
            start_watcher_with::<notify::PollWatcher>(&root, notify_config, sender).unwrap();
        // The dirty worktree is part of the watcher's baseline. The operation
        // below writes only .git (and the ignored .snip lock), so it must not
        // produce a new application event.
        std::thread::sleep(Duration::from_millis(150));
        app.tick_auto_backup();
        assert_eq!(crate::git::status(&repo).unwrap().dirty_count(), 0);
        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(750)),
            Err(RecvTimeoutError::Timeout)
        ));
    }
}
