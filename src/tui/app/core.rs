use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use ratatui::widgets::ListState;
use uuid::Uuid;

use crate::config::{AppConfig, TuiThemeSetting};
use crate::domain::{FolderFilter, Snippet};
use crate::error::Result;
use crate::filesystem::Library;
use crate::search::{MemoryIndex, SearchIndex, SearchQuery};
use crate::service::trash_entries;

use super::super::event::{AppEvent, GitTaskResult};
use super::super::highlight::Highlighter;
use super::super::icons::IconMode;
use super::super::layout::LayoutRects;
use super::super::preview::PreviewCache;
use super::super::selection::PreviewSelection;
use super::super::sidebar;
use super::super::state::{
    Filter, Pane, SearchState, SidebarState, StatusLevel, StatusMessage, VisibleRow,
};
use super::super::theme::TuiTheme;
use super::super::trash::TrashState;
use super::types::App;
use super::types::GitState;

impl App {
    pub fn new(library: Library, config: &AppConfig) -> Result<Self> {
        let catalog = library.scan()?;
        let index = MemoryIndex::new(catalog.clone());
        let tui = config.tui.clone().unwrap_or_default();
        let theme_overrides = tui
            .extra
            .get("colors")
            .and_then(toml::Value::as_table)
            .cloned()
            .unwrap_or_default();
        let theme = TuiTheme::resolve(tui.theme).with_overrides(&theme_overrides);
        let sort = tui.sort;
        let density = tui.density;
        let icon_mode = IconMode::Ascii;
        let mut app = Self {
            git: GitState::probe(&library, &config.git.clone().unwrap_or_default()),
            library,
            catalog,
            index,
            focus: Pane::Sidebar,
            sidebar: SidebarState::default(),
            filter: Filter::default(),
            search: SearchState::default(),
            visible: Vec::new(),
            list_state: ListState::default(),
            selected_id: None,
            fragment_index: 0,
            preview_scroll: 0,
            show_line_numbers: true,
            sort,
            density,
            layout: LayoutRects::default(),
            preview: PreviewCache::default(),
            preview_selection: PreviewSelection::default(),
            highlighter: Highlighter::new(theme)?,
            theme,
            theme_setting: tui.theme,
            theme_overrides,
            icon_mode,
            theme_checked_at: Instant::now(),
            status: None,
            modal: None,
            palette: Default::default(),
            trash: TrashState::default(),
            should_quit: false,
            pending_quit: false,
            editor_cmd: config.editor.clone(),
            vscode_cmd: config.vscode_cmd.clone(),
            show_help: false,
            help_scroll: 0,
            default_language: config
                .default_language
                .clone()
                .unwrap_or_else(|| "text".to_owned()),
            default_folder: config.default_folder.clone(),
            default_tags: config.default_tags.clone(),
            last_click: None,
        };
        let trash_count = trash_entries(&app.library).map_or(0, |entries| entries.len());
        sidebar::rebuild(&mut app.sidebar, &app.catalog, trash_count);
        app.refresh_visible();
        app.refresh_git();
        Ok(app)
    }

    pub fn selected_snippet(&self) -> Option<&Snippet> {
        let id = self.selected_id?;
        self.catalog
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
    }

    pub fn set_status(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.status = Some(StatusMessage::new(text, level));
    }

    pub fn tick_status(&mut self) {
        if self.modal.is_none() && self.status.as_ref().is_some_and(StatusMessage::expired) {
            self.status = None;
        }
    }

    pub fn tick_theme(&mut self) -> Result<()> {
        if self.theme_setting != TuiThemeSetting::Auto {
            return Ok(());
        }
        if self.theme_checked_at.elapsed() < Duration::from_secs(5) {
            return Ok(());
        }
        self.theme_checked_at = Instant::now();
        let theme = TuiTheme::resolve(self.theme_setting).with_overrides(&self.theme_overrides);
        if theme.appearance != self.theme.appearance {
            self.highlighter = Highlighter::new(theme)?;
            self.theme = theme;
            self.preview.invalidate();
        }
        Ok(())
    }

    pub fn tick_git(&mut self) {
        let Some(repo) = self.git.repo.as_ref() else {
            return;
        };
        if self.git.checked_at.elapsed() < self.git.interval
            || repo.git_dir.join("index.lock").exists()
        {
            return;
        }
        self.refresh_git();
    }

    pub fn tick_auto_backup(&mut self) {
        if self.should_quit
            || self.pending_quit
            || self.modal.is_some()
            || self.git.operation_queued
        {
            return;
        }

        if self
            .git
            .auto_attempted_at
            .is_none_or(|attempt| attempt.elapsed() >= Duration::from_secs(5))
            && let (Some(repo), Some(status)) = (self.git.repo.clone(), self.git.status.clone())
            && crate::git::should_auto_backup(
                &status,
                time::OffsetDateTime::now_utc().unix_timestamp(),
                self.git.auto_commit_interval,
                self.git.auto_backup_paused,
            )
        {
            let message = crate::git::backup_message(&status);
            self.git.auto_attempted_at = Some(Instant::now());
            // A push does not hold the index lock. Let commits continue while a
            // background push is in flight; a newer HEAD is pushed next time.
            match crate::git::commit(&repo, &message) {
                Ok(()) => {
                    self.git.last_commit_error = None;
                    // The push gate below must see the commit that just landed.
                    self.refresh_git();
                }
                Err(error) if crate::git::is_library_lock_conflict(&error) => {}
                Err(error) => {
                    let message = error.to_string();
                    if auto_error_transition(&mut self.git.last_commit_error, message.clone()) {
                        self.set_status(
                            format!("automatic commit failed: {message}"),
                            StatusLevel::Error,
                        );
                    }
                }
            }
        }

        if self.git.auto_commit_interval == 0
            || !self.git.auto_push
            || self.git.push_attempted_at.is_some_and(|attempt| {
                attempt.elapsed()
                    < Duration::from_secs(u64::from(self.git.auto_commit_interval) * 60)
            })
        {
            return;
        }
        self.spawn_auto_push();
    }

    pub fn set_git_sender(&mut self, sender: Sender<AppEvent>) {
        self.git.sender = Some(sender);
    }

    fn spawn_auto_push(&mut self) {
        let (Some(repo), Some(sender)) = (self.git.repo.clone(), self.git.sender.clone()) else {
            return;
        };
        let Some(status) = self.git.status.as_ref() else {
            return;
        };
        if self.git.push_in_flight
            || !crate::git::should_auto_push(
                status,
                self.git.auto_push,
                self.git.auto_backup_paused,
            )
        {
            return;
        }
        self.git.push_in_flight = true;
        self.git.push_attempted_at = Some(Instant::now());
        std::thread::spawn(move || {
            let outcome = crate::git::push(&repo)
                .map(|()| crate::git::ActionOutcome {
                    action: "push",
                    committed: false,
                    pushed: true,
                    message: "backup pushed".to_owned(),
                })
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::GitFinished(GitTaskResult {
                action: "push",
                outcome,
            }));
        });
    }

    pub fn spawn_fetch(&mut self) {
        let (Some(repo), Some(sender)) = (self.git.repo.clone(), self.git.sender.clone()) else {
            self.set_status("Git fetch is unavailable", StatusLevel::Error);
            return;
        };
        if self.git.push_in_flight || self.git.fetch_in_flight {
            self.set_status(
                "a background Git network task is running",
                StatusLevel::Error,
            );
            return;
        }
        self.git.fetch_in_flight = true;
        std::thread::spawn(move || {
            let outcome = crate::git::fetch(&repo)
                .map(|()| crate::git::ActionOutcome {
                    action: "fetch",
                    committed: false,
                    pushed: false,
                    message: "remote status refreshed".to_owned(),
                })
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::GitFinished(GitTaskResult {
                action: "fetch",
                outcome,
            }));
        });
    }

    pub fn handle_git_task(&mut self, result: GitTaskResult) {
        if result.action == "push" {
            self.git.push_in_flight = false;
        } else if result.action == "fetch" {
            self.git.fetch_in_flight = false;
        }
        self.refresh_git();
        match result.outcome {
            Ok(_) => {
                // Background success is visible in the badge and panel. Keep it
                // quiet so an interval backup does not become a recurring toast.
                if result.action == "fetch" {
                    self.git.last_fetch_error = None;
                    self.git.fetched_at = Some(Instant::now());
                    self.set_status("remote status refreshed", StatusLevel::Info);
                } else {
                    self.git.last_push_error = None;
                }
            }
            Err(message) => {
                let slot = if result.action == "fetch" {
                    &mut self.git.last_fetch_error
                } else {
                    &mut self.git.last_push_error
                };
                if auto_error_transition(slot, message.clone()) {
                    self.set_status(
                        format!("background {} failed: {message}", result.action),
                        StatusLevel::Error,
                    );
                }
            }
        }
    }

    pub fn refresh_git(&mut self) {
        let Some(repo) = self.git.repo.as_ref() else {
            return;
        };
        if repo.git_dir.join("index.lock").exists() {
            return;
        }
        let started = Instant::now();
        match crate::git::status(repo) {
            Ok(status) => {
                self.git.status = Some(status);
                self.git.error = None;
            }
            Err(error) => {
                // Checkouts can expose a transient state. Keep the last good badge.
                self.git.error = Some(error.to_string());
            }
        }
        self.git.checked_at = Instant::now();
        self.git.interval = if started.elapsed() > Duration::from_millis(250) {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(5)
        };
    }

    pub fn reprobe_git(&mut self) {
        self.git.reprobe(&self.library);
        self.refresh_git();
    }

    pub fn finish_git_operation(&mut self) {
        self.git.operation_queued = false;
        if self.pending_quit {
            self.should_quit = true;
        }
    }

    pub fn rescan(&mut self) -> Result<()> {
        let catalog = self.library.scan()?;
        self.catalog = catalog.clone();
        self.index = MemoryIndex::new(catalog);
        self.rebuild_sidebar();
        self.refresh_visible();
        if self.trash.open {
            self.trash.reload(&self.library)?;
        }
        self.refresh_git();
        Ok(())
    }

    pub(super) fn rescan_now(&mut self) {
        match self.rescan() {
            Ok(()) => self.set_status("library refreshed", StatusLevel::Info),
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub fn refresh_visible(&mut self) {
        let old_index = self.list_state.selected().unwrap_or(0);
        let allowed = self
            .catalog
            .snippets
            .iter()
            .filter(|snippet| self.matches_filter(snippet))
            .map(|snippet| snippet.id)
            .collect::<HashSet<_>>();

        let mut visible = if self.search.query.is_empty() {
            let mut snippets = self
                .catalog
                .snippets
                .iter()
                .filter(|snippet| allowed.contains(&snippet.id))
                .collect::<Vec<_>>();
            let sort = self.sort;
            snippets.sort_by(|left, right| sort.compare(left, right));
            snippets
                .into_iter()
                .map(|snippet| VisibleRow {
                    snippet_id: snippet.id,
                    excerpt: None,
                    score: 0,
                })
                .collect()
        } else {
            let mut best = HashMap::<Uuid, VisibleRow>::new();
            // The sidebar folder filter is applied separately through `allowed`,
            // so the query only carries the tag. Substring matching cannot fail,
            // hence the query is always constructible here.
            let query = SearchQuery::new(&self.search.query, false)
                .expect("substring queries never fail to build")
                .tag(self.filter.tag.as_deref());
            for result in self.index.search(&query) {
                if !allowed.contains(&result.snippet_id) {
                    continue;
                }
                best.entry(result.snippet_id).or_insert(VisibleRow {
                    snippet_id: result.snippet_id,
                    excerpt: Some(result.excerpt),
                    score: result.score,
                });
            }
            let mut rows = best.into_values().collect::<Vec<_>>();
            rows.sort_by(|left, right| {
                right.score.cmp(&left.score).then_with(|| {
                    self.title_for(left.snippet_id)
                        .to_lowercase()
                        .cmp(&self.title_for(right.snippet_id).to_lowercase())
                })
            });
            rows
        };

        let selection = self
            .selected_id
            .and_then(|id| visible.iter().position(|row| row.snippet_id == id))
            .or_else(|| (!visible.is_empty()).then(|| old_index.min(visible.len() - 1)));
        self.list_state.select(selection);
        self.selected_id = selection.map(|index| visible[index].snippet_id);
        self.visible.clear();
        self.visible.append(&mut visible);
        self.clamp_fragment();
        self.preview.invalidate();
        self.preview_scroll = 0;
    }

    pub(super) fn rebuild_sidebar(&mut self) {
        let trash_count = trash_entries(&self.library).map_or(0, |entries| entries.len());
        sidebar::rebuild(&mut self.sidebar, &self.catalog, trash_count);
    }

    pub(super) fn clamp_fragment(&mut self) {
        let count = self
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        self.fragment_index = self.fragment_index.min(count.saturating_sub(1));
    }

    pub(super) fn title_for(&self, id: Uuid) -> &str {
        self.catalog
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
            .map_or("", |snippet| snippet.title.as_str())
    }

    pub(super) fn matches_filter(&self, snippet: &Snippet) -> bool {
        if self.filter.uncategorized {
            return snippet.folder.is_empty();
        }
        let folder_matches = self
            .filter
            .folder
            .as_deref()
            .is_none_or(|folder| FolderFilter::recursive(folder).matches(&snippet.folder));
        let tag_matches = self.filter.tag.as_ref().is_none_or(|tag| {
            snippet
                .tags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(tag))
        });
        folder_matches && tag_matches
    }
}

fn auto_error_transition(last: &mut Option<String>, next: String) -> bool {
    if last.as_deref() == Some(next.as_str()) {
        false
    } else {
        *last = Some(next);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::auto_error_transition;

    #[test]
    fn repeated_auto_backup_errors_are_silent_until_the_message_changes() {
        let mut last = None;
        assert!(auto_error_transition(&mut last, "first".to_owned()));
        assert!(!auto_error_transition(&mut last, "first".to_owned()));
        assert!(auto_error_transition(&mut last, "second".to_owned()));
        assert_eq!(last.as_deref(), Some("second"));
    }
}
