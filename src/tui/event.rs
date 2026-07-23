use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify_debouncer_mini::{Config, DebounceEventResult, Debouncer, new_debouncer_opt};

use crate::error::{Result, SnipError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppEvent {
    FsChanged,
}

pub struct WatchHandle {
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

pub fn start_watcher(root: &Path) -> Result<(WatchHandle, Receiver<AppEvent>)> {
    let (debouncer, receiver) =
        start_watcher_with::<notify::RecommendedWatcher>(root, notify::Config::default())?;
    Ok((
        WatchHandle {
            _debouncer: debouncer,
        },
        receiver,
    ))
}

fn start_watcher_with<T: Watcher>(
    root: &Path,
    notify_config: notify::Config,
) -> Result<(Debouncer<T>, Receiver<AppEvent>)> {
    let root = root.to_path_buf();
    let (sender, receiver) = mpsc::channel();
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
    Ok((debouncer, receiver))
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
    use std::time::Duration;

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
        let (_watcher, receiver) =
            start_watcher_with::<notify::PollWatcher>(temporary.path(), notify_config).unwrap();
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
        let (_watcher, _receiver) = start_watcher(root).unwrap();
    }
}
