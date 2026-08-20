use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc::Sender};
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
use super::super::persist::SessionState;
use super::super::preview::{PreviewCache, PreviewTarget, has_readme};
use super::super::selection::PreviewSelection;
use super::super::sidebar;
use super::super::state::{
    Filter, Pane, SearchState, SidebarState, StatusLevel, StatusMessage, VisibleRow,
};
use super::super::theme::{Appearance, TuiTheme};
use super::super::trash::TrashState;
use super::types::{App, GistState, GitState, ThemePreviewState};

impl App {
    /// Builds an app with no carried-over session state.
    ///
    /// Loading the on-disk state belongs to the real entry point, not to a
    /// constructor: reading it here would make every caller — tests included —
    /// silently inherit whatever the developer's own `state.toml` holds.
    pub fn new(library: Library, config: &AppConfig) -> Result<Self> {
        Self::new_with_session_state(library, config, SessionState::default())
    }

    pub(crate) fn new_with_session_state(
        library: Library,
        config: &AppConfig,
        session_state: SessionState,
    ) -> Result<Self> {
        Self::new_with_keymap(
            library,
            config,
            session_state,
            crate::keys::Keymap::defaults(),
            0,
        )
    }

    pub(crate) fn new_with_keymap(
        library: Library,
        config: &AppConfig,
        session_state: SessionState,
        keymap: crate::keys::Keymap,
        key_error_count: usize,
    ) -> Result<Self> {
        let SessionState {
            recent_commands,
            extra,
            ..
        } = session_state;
        let catalog = Arc::new(library.scan()?);
        let index = MemoryIndex::new(Arc::clone(&catalog));
        let gist_badges = crate::tui::gist_panel::compute_all(&catalog.snippets);
        let tui = config.tui.clone().unwrap_or_default();
        let theme_overrides = tui
            .extra
            .get("colors")
            .and_then(toml::Value::as_table)
            .cloned()
            .unwrap_or_default();
        let theme_env = std::env::var("SNIP_TUI_THEME").ok();
        let (theme_source, theme_warnings) =
            Self::resolve_theme_for(&tui, None, theme_env.as_deref());
        let theme_name = theme_source.name.clone();
        let theme = TuiTheme::from(&theme_source).with_overrides(&theme_overrides);
        let sort = tui.sort;
        let density = tui.density;
        let show_line_numbers = tui.line_numbers;
        let simplified_ui = tui.simplified_ui;
        let icon_mode = IconMode::Ascii;
        let mut app = Self {
            config_path: crate::config::config_path()?,
            git: GitState::probe(&library, &config.git.clone().unwrap_or_default()),
            gist: GistState::default(),
            library,
            catalog,
            index,
            gist_badges,
            focus: Pane::Sidebar,
            sidebar: SidebarState::default(),
            filter: Filter::default(),
            search: SearchState::default(),
            visible: Vec::new(),
            list_state: ListState::default(),
            selected_id: None,
            preview_target: PreviewTarget::default(),
            fragment_grab: None,
            preview_scroll: 0,
            show_line_numbers,
            simplified_ui,
            show_sidebar: true,
            fragments_expanded: false,
            sort,
            density,
            layout: LayoutRects::default(),
            preview: PreviewCache::default(),
            preview_selection: PreviewSelection::default(),
            highlighter: Highlighter::new(&theme_source)?,
            theme,
            theme_source,
            theme_name,
            theme_preview: None,
            theme_setting: tui.theme,
            theme_config: tui.clone(),
            theme_overrides,
            appearance_override: None,
            theme_env,
            icon_mode,
            theme_checked_at: Instant::now(),
            status: None,
            modal: None,
            palette: Default::default(),
            keymap,
            session_state_extra: extra,
            trash: TrashState::default(),
            should_quit: false,
            pending_quit: false,
            editor_cmd: config.editor.clone(),
            editor_cwd: config.editor_cwd.unwrap_or_default(),
            vscode_cmd: config.vscode_cmd.clone(),
            show_help: false,
            help: Default::default(),
            default_language: config
                .default_language
                .clone()
                .unwrap_or_else(|| "text".to_owned()),
            default_folder: config.default_folder.clone(),
            default_tags: config.default_tags.clone(),
            last_click: None,
            #[cfg(test)]
            last_command: None,
        };
        app.palette.set_recent(
            recent_commands
                .iter()
                .filter_map(|slug| crate::tui::command::by_slug(slug))
                .collect(),
        );
        let trash_count = trash_entries(&app.library).map_or(0, |entries| entries.len());
        sidebar::rebuild(&mut app.sidebar, &app.catalog, trash_count);
        app.refresh_visible();
        app.refresh_git();
        if !theme_warnings.is_empty() {
            app.set_status(theme_warnings.join("; "), StatusLevel::Error);
        }
        if key_error_count > 0 {
            let noun = if key_error_count == 1 {
                "binding"
            } else {
                "bindings"
            };
            app.set_status(
                format!("{key_error_count} key {noun} ignored; run \"snip keys check\""),
                StatusLevel::Error,
            );
        }
        Ok(app)
    }

    pub(crate) fn session_state(&self) -> SessionState {
        SessionState {
            schema_version: crate::tui::persist::STATE_SCHEMA_VERSION,
            recent_commands: self
                .palette
                .recent()
                .iter()
                .map(|id| crate::tui::command::get(*id).slug.to_owned())
                .collect(),
            extra: self.session_state_extra.clone(),
        }
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

    pub(super) fn preview_theme(&mut self, name: &str) -> Result<()> {
        if self.theme_preview.is_none() {
            self.theme_preview = Some(ThemePreviewState {
                original_name: self.theme_name.clone(),
                original_source: self.theme_source.clone(),
                original_tui: self.theme,
            });
        }
        let source = crate::theme::load(name)?;
        if let Some(failure) = crate::theme::validate::check(&source)
            .into_iter()
            .find(|check| check.level == crate::theme::validate::Level::Fail)
        {
            return Err(crate::error::SnipError::validation(format!(
                "theme {name}: {}: {}",
                failure.id, failure.detail
            )));
        }
        // `[tui.colors]` is the last layer for every other path that builds a
        // theme, so it has to be reapplied here too. Without it, switching
        // themes from the palette would silently drop the user's overrides for
        // the rest of the session.
        let theme = TuiTheme::from(&source).with_overrides(&self.theme_overrides);
        self.highlighter.set_theme(&source)?;
        self.theme_source = source;
        self.theme = theme;
        self.theme_name = name.to_owned();
        self.preview.invalidate();
        Ok(())
    }

    pub(super) fn restore_theme_preview(&mut self) -> Result<()> {
        let Some(original) = self.theme_preview.take() else {
            return Ok(());
        };
        self.highlighter.set_theme(&original.original_source)?;
        self.theme_name = original.original_name;
        self.theme_source = original.original_source;
        self.theme = original.original_tui;
        self.preview.invalidate();
        Ok(())
    }

    pub fn tick_status(&mut self) {
        if self.modal.is_none() && self.status.as_ref().is_some_and(StatusMessage::expired) {
            self.status = None;
        }
    }

    pub fn tick_theme(&mut self) -> Result<()> {
        // An explicit session override already occupies the environment slot in
        // resolution. Re-probing the host every five seconds would only spawn a
        // subprocess whose answer is discarded.
        if self.theme_setting != TuiThemeSetting::Auto
            || self.appearance_override.is_some()
            || self.theme_preview.is_some()
        {
            return Ok(());
        }
        if self.theme_checked_at.elapsed() < Duration::from_secs(5) {
            return Ok(());
        }
        self.theme_checked_at = Instant::now();
        self.theme_config.theme = self.theme_setting;
        self.apply_resolved_theme()
    }

    fn resolve_theme_for(
        config: &crate::config::TuiConfig,
        appearance_override: Option<Appearance>,
        theme_env: Option<&str>,
    ) -> (crate::theme::Theme, Vec<String>) {
        let env = appearance_override.map(Appearance::as_str).or(theme_env);
        crate::theme::resolve_with_environment(config, env)
    }

    fn resolve_theme(&self) -> (crate::theme::Theme, Vec<String>) {
        Self::resolve_theme_for(
            &self.theme_config,
            self.appearance_override,
            self.theme_env.as_deref(),
        )
    }

    fn apply_resolved_theme(&mut self) -> Result<()> {
        let (source, warnings) = self.resolve_theme();
        let theme = TuiTheme::from(&source).with_overrides(&self.theme_overrides);
        if source.name != self.theme_name || theme.appearance != self.theme.appearance {
            self.highlighter.set_theme(&source)?;
            self.theme_name = source.name.clone();
            self.theme_source = source;
            self.theme = theme;
            self.preview.invalidate();
            // Warnings are feedback for an actual theme change; re-emitting them
            // every tick would pin a message over the bottom bar forever.
            if !warnings.is_empty() {
                self.set_status(warnings.join("; "), StatusLevel::Error);
            }
        }
        Ok(())
    }

    pub(super) fn cycle_appearance(&mut self) {
        let next = match self.appearance_override {
            None => match self.theme.appearance {
                Appearance::Dark => Appearance::Light,
                Appearance::Light => Appearance::Dark,
            },
            Some(Appearance::Light) => Appearance::Dark,
            Some(Appearance::Dark) => Appearance::Light,
        };
        self.appearance_override = Some(next);
        self.set_status(
            format!("appearance: {} (this session)", next.as_str()),
            StatusLevel::Info,
        );
        if let Err(error) = self.apply_resolved_theme() {
            self.set_status(
                format!("appearance changed for this session: {error}"),
                StatusLevel::Error,
            );
        }
    }

    pub(super) fn clear_appearance_override(&mut self) {
        self.appearance_override = None;
        self.set_status("appearance override cleared", StatusLevel::Info);
        if let Err(error) = self.apply_resolved_theme() {
            self.set_status(
                format!("could not clear appearance override: {error}"),
                StatusLevel::Error,
            );
        }
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
            || self.git.pull_in_flight
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
                    pulled: 0,
                    message: "backup pushed".to_owned(),
                })
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::GitFinished(GitTaskResult {
                action: "push",
                outcome,
                manual: false,
            }));
        });
    }

    pub fn spawn_fetch(&mut self) {
        let (Some(repo), Some(sender)) = (self.git.repo.clone(), self.git.sender.clone()) else {
            self.set_status("Git fetch is unavailable", StatusLevel::Error);
            return;
        };
        if self.git.push_in_flight || self.git.pull_in_flight || self.git.fetch_in_flight {
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
                    pulled: 0,
                    message: "remote status refreshed".to_owned(),
                })
                .map_err(|error| error.to_string());
            let _ = sender.send(AppEvent::GitFinished(GitTaskResult {
                action: "fetch",
                outcome,
                manual: false,
            }));
        });
    }

    pub fn spawn_auto_pull(&mut self) {
        if !self.git.auto_pull || self.git.pull_in_flight {
            return;
        }
        let (Some(repo), Some(sender), Some(status)) = (
            self.git.repo.clone(),
            self.git.sender.clone(),
            self.git.status.as_ref(),
        ) else {
            return;
        };
        if crate::git::check_pull(status, status.dirty_count()).is_err() {
            return;
        }
        let upstream = status.upstream.clone().unwrap_or_else(|| "@{u}".to_owned());
        self.git.pull_in_flight = true;
        std::thread::spawn(move || {
            let outcome = match crate::git::pull(&repo, false) {
                Ok(pull) => Ok(crate::git::ActionOutcome {
                    action: "pull",
                    committed: false,
                    pushed: false,
                    pulled: pull.pulled,
                    message: crate::git::pull_message(&pull, &upstream),
                }),
                Err(error) if crate::git::is_pull_refusal(&error) => {
                    Ok(crate::git::ActionOutcome {
                        action: "pull",
                        committed: false,
                        pushed: false,
                        pulled: 0,
                        message: String::new(),
                    })
                }
                Err(error) => Err(error.to_string()),
            };
            let _ = sender.send(AppEvent::GitFinished(GitTaskResult {
                action: "pull",
                outcome,
                manual: false,
            }));
        });
    }

    /// Spawns a manual git operation (backup, commit, push, pull) as a background
    /// task. The caller must have already set `operation_queued = true` and
    /// validated preconditions. Results arrive via `AppEvent::GitFinished`.
    pub(super) fn spawn_git_operation(&mut self, action: crate::git::GitAction) {
        let (action_name, pending) = match &action {
            crate::git::GitAction::Backup => ("backup", "backing up…"),
            crate::git::GitAction::Commit { .. } => ("commit", "committing…"),
            crate::git::GitAction::Push => ("push", "pushing…"),
            crate::git::GitAction::Pull => ("pull", "pulling…"),
            crate::git::GitAction::Init => {
                debug_assert!(false, "Init must use the interactive RunGit effect");
                self.set_status("git init requires the interactive flow", StatusLevel::Error);
                self.finish_git_operation();
                return;
            }
        };
        let (Some(repo), Some(sender)) = (self.git.repo.clone(), self.git.sender.clone()) else {
            self.set_status("Git is unavailable", StatusLevel::Error);
            self.finish_git_operation();
            return;
        };
        if matches!(action, crate::git::GitAction::Pull) {
            self.git.pull_in_flight = true;
        }
        self.set_status(pending, StatusLevel::Info);
        std::thread::spawn(move || {
            let outcome =
                crate::git::execute_non_interactive(&repo, &action).map_err(|e| e.to_string());
            let _ = sender.send(AppEvent::GitFinished(GitTaskResult {
                action: action_name,
                outcome,
                manual: true,
            }));
        });
    }

    pub fn handle_git_task(&mut self, result: GitTaskResult) {
        if result.action == "push" && !result.manual {
            self.git.push_in_flight = false;
        } else if result.action == "pull" {
            self.git.pull_in_flight = false;
        } else if result.action == "fetch" {
            self.git.fetch_in_flight = false;
        }
        let pulled = result.outcome.as_ref().map_or(0, |outcome| outcome.pulled);
        if pulled > 0 {
            if let Err(error) = self.rescan() {
                self.set_status(error.to_string(), StatusLevel::Error);
            }
        } else {
            self.refresh_git();
        }
        if result.manual {
            // Manual user-triggered operation: always show result.
            match &result.outcome {
                Ok(outcome) => {
                    // A successful manual push retires the stale panel banner
                    // left behind by an earlier automatic push failure.
                    if outcome.pushed {
                        self.git.last_push_error = None;
                    }
                    self.set_status(&outcome.message, StatusLevel::Info);
                }
                Err(message) => self.set_status(message, StatusLevel::Error),
            }
            self.finish_git_operation();
        } else {
            // Automatic background task: quiet on success, throttled on error.
            match result.outcome {
                Ok(outcome) => {
                    if result.action == "fetch" {
                        self.git.last_fetch_error = None;
                        self.git.fetched_at = Some(Instant::now());
                        self.set_status("remote status refreshed", StatusLevel::Info);
                    } else if result.action == "pull" {
                        if outcome.pulled > 0 {
                            self.set_status(&outcome.message, StatusLevel::Info);
                        }
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
        let catalog = Arc::new(self.library.scan()?);
        self.index = MemoryIndex::new(Arc::clone(&catalog));
        self.catalog = catalog;
        self.gist_badges = crate::tui::gist_panel::compute_all(&self.catalog.snippets);
        self.rebuild_sidebar();
        self.refresh_visible();
        if self.trash.open {
            self.trash.reload(&self.library)?;
            self.sync_trash_preview();
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
            .or_else(|| (!visible.is_empty()).then_some(0));
        // `ListState::select` does not update the viewport offset. Clamp a
        // stale offset far enough to fill the viewport, while preserving a
        // still-valid offset so background rescans do not move the cursor row.
        let viewport_rows = self.layout.list.height.saturating_sub(2) / self.density.row_height();
        let max_offset = visible.len().saturating_sub(viewport_rows as usize);
        *self.list_state.offset_mut() = self.list_state.offset().min(max_offset);
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
        let viewport_rows = self.layout.sidebar.height.saturating_sub(2) as usize;
        let max_offset = self.sidebar.rows.len().saturating_sub(viewport_rows);
        *self.sidebar.list_state.offset_mut() = self.sidebar.list_state.offset().min(max_offset);
    }

    /// Re-anchors the preview target after a rescan. A README selection
    /// survives as long as the README does — it only falls back when the file
    /// was deleted out from under us.
    pub(super) fn clamp_fragment(&mut self) {
        let snippet = self.selected_snippet();
        let count = snippet.map_or(0, |snippet| snippet.loaded_fragments.len());
        let readme = snippet.is_some_and(has_readme);
        self.preview_target = match self.preview_target {
            PreviewTarget::Fragment(index) => {
                PreviewTarget::Fragment(index.min(count.saturating_sub(1)))
            }
            PreviewTarget::Readme if readme => PreviewTarget::Readme,
            PreviewTarget::Readme => PreviewTarget::Fragment(0),
        };
    }

    pub(super) fn title_for(&self, id: Uuid) -> &str {
        self.catalog
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
            .map_or("", |snippet| snippet.title.as_str())
    }

    pub(super) fn matches_filter(&self, snippet: &Snippet) -> bool {
        if self.filter.published && crate::gist::find(snippet).is_none() {
            return false;
        }
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
    use super::*;
    use crate::config::AppConfig;
    use crate::domain::{Fingerprint, RemoteRecord, Snippet, SnippetManifest};
    use crate::tui::command::CommandId;
    use crate::tui::persist::SessionState;

    fn make_test_snippet(folder: &str, gist: bool) -> Snippet {
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Test".to_owned(),
                tags: vec![],
                pinned: false,
                locked: false,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                source: None,
                remotes: gist
                    .then(|| RemoteRecord {
                        kind: "gist".to_owned(),
                        host: "github.com".to_owned(),
                        id: "5b0e0062eb8e9654adad7bb1d81cc75f".to_owned(),
                        url: "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f"
                            .to_owned(),
                        public: false,
                        description: None,
                        files: vec![],
                        include_notes: false,
                        include_readme: true,
                        pushed_at: None,
                        pushed_digest: None,
                        extra: toml::Table::new(),
                    })
                    .into_iter()
                    .collect(),
                fragments: vec![],
                extra: toml::Table::new(),
            },
            readme: None,
            folder: folder.to_owned(),
            package_path: std::path::PathBuf::new(),
            modified_at: None,
            fingerprint: Fingerprint("abc".to_owned()),
            loaded_fragments: vec![],
        }
    }

    /// An app over a one-fragment snippet, with or without a README.
    fn app_with_readme(readme: Option<&str>) -> (tempfile::TempDir, App) {
        let temporary = tempfile::tempdir().unwrap();
        let library =
            crate::filesystem::library::Library::init(&temporary.path().join("T.sniplib"), None)
                .unwrap();
        crate::service::create_snippet(
            &library,
            &crate::service::CreateOptions {
                title: "Snippet".to_owned(),
                language: "rust".to_owned(),
                content: "let value = 1;\n".to_owned(),
                readme: readme.map(str::to_owned),
                ..crate::service::CreateOptions::default()
            },
        )
        .unwrap();
        let app = App::new(library, &AppConfig::default()).unwrap();
        (temporary, app)
    }

    fn replace_catalog(app: &mut App, snippets: Vec<Snippet>) {
        let mut catalog = app.library.scan().unwrap();
        catalog.snippets = snippets;
        let catalog = Arc::new(catalog);
        app.index = MemoryIndex::new(Arc::clone(&catalog));
        app.catalog = catalog;
    }

    #[test]
    fn app_and_search_index_share_the_catalog_allocation() {
        let (_temporary, app) = app_with_readme(None);

        assert!(Arc::ptr_eq(&app.catalog, app.index.catalog_arc()));
    }

    #[test]
    fn rescan_keeps_the_readme_target() {
        let (_temporary, mut app) = app_with_readme(Some("snippet level prose\n"));
        app.preview_target = PreviewTarget::Readme;
        app.clamp_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Readme);
    }

    #[test]
    fn rescan_drops_a_vanished_readme() {
        let (_temporary, mut app) = app_with_readme(None);
        app.preview_target = PreviewTarget::Readme;
        app.clamp_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Fragment(0));
    }

    #[test]
    fn repeated_auto_backup_errors_are_silent_until_the_message_changes() {
        let mut last = None;
        assert!(auto_error_transition(&mut last, "first".to_owned()));
        assert!(!auto_error_transition(&mut last, "first".to_owned()));
        assert!(auto_error_transition(&mut last, "second".to_owned()));
        assert_eq!(last.as_deref(), Some("second"));
    }

    #[test]
    fn persisted_recent_commands_round_trip_and_ignore_unknown_slugs() {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Recent.sniplib"), None).unwrap();
        let mut app = App::new(library.clone(), &AppConfig::default()).unwrap();
        // Both commands are session-only. A command that persists (density,
        // line numbers) would rewrite the developer's real config.toml.
        app.run_command(CommandId::ViewCycleSort);
        app.run_command(CommandId::ViewToggleHelp);
        let path = temporary.path().join("state.toml");
        let mut extra = toml::Table::new();
        extra.insert(
            "future_setting".to_owned(),
            toml::Value::String("kept".to_owned()),
        );
        SessionState {
            recent_commands: app
                .palette
                .recent()
                .iter()
                .map(|id| crate::tui::command::get(*id).slug.to_owned())
                .chain(std::iter::once("snippet.does-not-exist".to_owned()))
                .collect(),
            extra,
            ..SessionState::default()
        }
        .save_to(&path)
        .unwrap();
        let state = SessionState::load_from(&path);
        let mut reopened =
            App::new_with_session_state(library, &AppConfig::default(), state).unwrap();
        reopened.palette.open();
        reopened.refresh_palette();
        assert_eq!(reopened.palette.matches[0].id, CommandId::ViewToggleHelp);
        assert_eq!(reopened.palette.matches[1].id, CommandId::ViewCycleSort);
        assert_eq!(reopened.palette.recent().len(), 2);
        reopened.session_state().save_to(&path).unwrap();
        assert_eq!(
            SessionState::load_from(&path)
                .extra
                .get("future_setting")
                .and_then(toml::Value::as_str),
            Some("kept")
        );
    }

    #[test]
    fn user_keymap_and_diagnostic_summary_are_applied_at_startup() {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Keys.sniplib"), None).unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(
            &path,
            r#"
                [global]
                "app.quit" = "x"

                [search]
                "snippet.new" = "n"
            "#,
        )
        .unwrap();
        let (keymap, diagnostics) = crate::keys::Keymap::load_from(&path).unwrap();
        let error_count = crate::tui::key_error_count(&diagnostics);

        let app = App::new_with_keymap(
            library,
            &AppConfig::default(),
            SessionState::default(),
            keymap,
            error_count,
        )
        .unwrap();

        assert_eq!(
            app.keymap
                .resolve(&[crate::keys::Mode::Global], "x".parse().unwrap()),
            Some(CommandId::AppQuit)
        );
        assert_eq!(
            app.keymap
                .resolve(&[crate::keys::Mode::Global], "q".parse().unwrap()),
            None
        );
        let status = app.status.as_ref().unwrap();
        assert_eq!(
            status.text,
            "1 key binding ignored; run \"snip keys check\""
        );
        assert_eq!(status.level, StatusLevel::Error);
    }

    #[test]
    fn info_only_key_diagnostics_do_not_raise_a_startup_error() {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Keys.sniplib"), None).unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(
            &path,
            r#"
                [list]
                "snippet.edit-content" = "m"
            "#,
        )
        .unwrap();
        let (keymap, diagnostics) = crate::keys::Keymap::load_from(&path).unwrap();
        let error_count = crate::tui::key_error_count(&diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, crate::keys::DiagnosticLevel::Info);
        assert_eq!(error_count, 0);

        let app = App::new_with_keymap(
            library,
            &AppConfig::default(),
            SessionState::default(),
            keymap,
            error_count,
        )
        .unwrap();

        assert_eq!(
            app.keymap
                .resolve(&[crate::keys::Mode::List], "m".parse().unwrap()),
            Some(CommandId::SnippetEditContent)
        );
        assert!(app.status.is_none());
    }

    #[test]
    fn refreshing_a_short_list_keeps_and_selects_its_first_row() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::widgets::List;

        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Offset.sniplib"), None).unwrap();
        let mut app = App::new(library, &AppConfig::default()).unwrap();
        app.sort = crate::tui::state::SortMode::Title;
        let mut first = make_test_snippet("Dotfiles", false);
        first.manifest.title = "First Dotfile".to_owned();
        let mut second = make_test_snippet("Dotfiles", false);
        second.manifest.title = "Second Dotfile".to_owned();
        let first_id = first.id;
        let mut outside = make_test_snippet("Other", false);
        outside.manifest.title = "Outside".to_owned();
        app.selected_id = Some(outside.id);
        replace_catalog(&mut app, vec![first, second, outside]);
        app.filter.folder = Some("Dotfiles".to_owned());
        app.list_state.select(Some(1));
        *app.list_state.offset_mut() = 25;
        app.layout.list = Rect::new(0, 0, 40, 6);

        app.refresh_visible();

        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
        terminal
            .draw(|frame| {
                let items = crate::tui::snippet_list::items(&app, 37);
                let title = format!("Snippets ({})", app.visible.len());
                frame.render_stateful_widget(
                    List::new(items)
                        .block(crate::tui::widgets::pane_block(
                            &title,
                            app.focus == Pane::List,
                            app.theme,
                        ))
                        .highlight_symbol(" "),
                    frame.area(),
                    &mut app.list_state,
                );
            })
            .unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("First Dotfile"));
        assert!(rendered.contains("Second Dotfile"));
        assert_eq!(app.selected_id, Some(first_id));
    }

    #[test]
    fn refreshing_an_unchanged_list_preserves_the_rendered_viewport() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use ratatui::widgets::List;

        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Stable Offset.sniplib"), None).unwrap();
        let mut app = App::new(library, &AppConfig::default()).unwrap();
        app.sort = crate::tui::state::SortMode::Title;
        replace_catalog(
            &mut app,
            (0..59)
                .map(|index| {
                    let mut snippet = make_test_snippet("", false);
                    snippet.manifest.title = format!("Snippet {index:02}");
                    snippet
                })
                .collect(),
        );
        app.layout.list = Rect::new(0, 0, 40, 42);
        app.refresh_visible();
        app.list_state.select(Some(40));
        app.selected_id = Some(app.visible[40].snippet_id);
        *app.list_state.offset_mut() = 30;

        let render = |app: &mut App| {
            let mut terminal = Terminal::new(TestBackend::new(40, 42)).unwrap();
            terminal
                .draw(|frame| {
                    let items = crate::tui::snippet_list::items(app, 37);
                    let title = format!("Snippets ({})", app.visible.len());
                    frame.render_stateful_widget(
                        List::new(items)
                            .block(crate::tui::widgets::pane_block(
                                &title,
                                app.focus == Pane::List,
                                app.theme,
                            ))
                            .highlight_symbol(" "),
                        frame.area(),
                        &mut app.list_state,
                    );
                })
                .unwrap();
            terminal.backend().buffer().clone()
        };
        let before = render(&mut app);

        app.refresh_visible();
        let after = render(&mut app);

        assert_eq!(before, after);
    }

    #[test]
    fn rebuilding_sidebar_clamps_only_an_excess_viewport_offset() {
        use ratatui::layout::Rect;

        let temporary = tempfile::tempdir().unwrap();
        let library =
            Library::init(&temporary.path().join("Sidebar Offset.sniplib"), None).unwrap();
        let mut app = App::new(library, &AppConfig::default()).unwrap();
        app.layout.sidebar = Rect::new(0, 0, 20, 5);
        *app.sidebar.list_state.offset_mut() = 25;

        app.rebuild_sidebar();

        let max_offset = app.sidebar.rows.len().saturating_sub(3);
        assert_eq!(app.sidebar.list_state.offset(), max_offset);

        *app.sidebar.list_state.offset_mut() = 2;
        app.rebuild_sidebar();
        assert_eq!(app.sidebar.list_state.offset(), 2);
    }

    #[test]
    fn published_filter_composes_with_the_folder_filter() {
        let mut app = App::new(
            Library::init(
                &tempfile::tempdir().unwrap().path().join("Filter.sniplib"),
                None,
            )
            .unwrap(),
            &AppConfig::default(),
        )
        .unwrap();
        app.filter.published = true;
        app.filter.folder = Some("Code/Rust".to_owned());

        let published = make_test_snippet("Code/Rust", true);
        let unpublished = make_test_snippet("Code/Rust", false);
        let published_elsewhere = make_test_snippet("Code/Shell", true);

        assert!(
            app.matches_filter(&published),
            "a published snippet inside the folder must pass"
        );
        assert!(
            !app.matches_filter(&unpublished),
            "an unpublished snippet must be excluded"
        );
        assert!(
            !app.matches_filter(&published_elsewhere),
            "a published snippet outside the folder must still be excluded"
        );
    }

    #[test]
    fn published_check_precedes_the_uncategorized_early_return() {
        let mut app = App::new(
            Library::init(
                &tempfile::tempdir().unwrap().path().join("Filter.sniplib"),
                None,
            )
            .unwrap(),
            &AppConfig::default(),
        )
        .unwrap();
        app.filter.published = true;
        app.filter.uncategorized = true;

        let root_published = make_test_snippet("", true);
        let root_unpublished = make_test_snippet("", false);

        assert!(app.matches_filter(&root_published));
        assert!(
            !app.matches_filter(&root_unpublished),
            "the published check must run before the uncategorized early return"
        );
    }
}
