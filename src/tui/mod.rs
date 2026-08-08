pub mod app;
pub mod bottom_bar;
pub mod command;
pub mod editor;
pub mod event;
pub mod gist_panel;
pub mod git_panel;
pub mod help;
pub mod highlight;
pub mod icons;
pub mod layout;
pub mod modal;
pub mod palette;
pub mod panel_text;
pub mod persist;
pub mod preview;
pub mod selection;
pub mod sidebar;
pub mod snippet_list;
pub mod state;
pub mod theme;
pub mod top_bar;
pub mod trash;
pub mod ui;
pub mod widgets;

use std::io::{self, IsTerminal, Stdout, Write};
use std::panic;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self as terminal_event, DisableMouseCapture, EnableMouseCapture, Event,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::config::AppConfig;
use crate::error::{Result, SnipError};
use crate::filesystem::Library;

use self::app::{App, Effect};
use self::editor::EditOutcome;
use self::state::StatusLevel;
use crate::clipboard::ClipboardMethod;

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub(crate) fn key_error_count(diagnostics: &[crate::keys::Diagnostic]) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == crate::keys::DiagnosticLevel::Error)
        .count()
}

pub fn run(library: Library, config: &AppConfig) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(SnipError::usage("the TUI requires an interactive terminal"));
    }
    let _panic_hook = PanicHookGuard::install();
    let mut guard = TerminalGuard::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (keymap, key_diagnostics) = crate::keys::Keymap::load()?;
    let key_error_count = key_error_count(&key_diagnostics);
    let mut app = App::new_with_keymap(
        library,
        config,
        persist::SessionState::load(),
        keymap,
        key_error_count,
    )?;
    let (sender, receiver) = mpsc::channel();
    app.set_git_sender(sender.clone());
    app.set_gist_sender(sender.clone());
    app.spawn_auto_pull();
    let _watcher = event::start_watcher(app.library.root(), sender)?;

    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        let mut effects = Vec::new();
        if terminal_event::poll(Duration::from_millis(120))? {
            match terminal_event::read()? {
                Event::Key(key) if key.kind == ratatui::crossterm::event::KeyEventKind::Press => {
                    effects.extend(app.handle_key(key));
                }
                Event::Mouse(mouse) => effects.extend(app.handle_mouse(mouse)),
                _ => {}
            }
        }
        let mut dirty = false;
        while let Ok(event) = receiver.try_recv() {
            handle_app_event(&mut app, event, &mut dirty);
        }
        // `pending_quit` also marks an already-queued quit backup. Only the
        // background-push form has no manual Git operation queued yet.
        if app.pending_quit && !app.git.operation_queued {
            if app.git.push_in_flight
                || app.git.pull_in_flight
                || app.git.fetch_in_flight
                || app.gist.in_flight
            {
                app.set_status(
                    if app.gist.in_flight {
                        "finishing gist operation…"
                    } else {
                        "finishing background Git task…"
                    },
                    StatusLevel::Info,
                );
                terminal.draw(|frame| ui::draw(frame, &mut app))?;
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    match receiver.recv_timeout(remaining) {
                        Ok(event::AppEvent::FsChanged) => dirty = true,
                        Ok(event::AppEvent::GitFinished(result)) => {
                            app.handle_git_task(result);
                            effects.extend(app.resume_quit_after_push());
                            break;
                        }
                        Ok(event::AppEvent::GistFinished(result)) => {
                            app.handle_gist_task(result);
                            effects.extend(app.resume_quit_after_push());
                            break;
                        }
                        Err(
                            mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected,
                        ) => {
                            // The worker owns no library data. If it cannot finish
                            // promptly, leave the local commits for the next session.
                            app.pending_quit = false;
                            app.should_quit = true;
                            break;
                        }
                    }
                }
            } else {
                effects.extend(app.resume_quit_after_push());
            }
        }
        if dirty && let Err(error) = app.rescan() {
            app.set_status(error.to_string(), StatusLevel::Error);
        }
        app.tick_status();
        if let Err(error) = app.tick_theme() {
            app.set_status(error.to_string(), StatusLevel::Error);
        }
        app.tick_git();
        app.tick_auto_backup();
        for effect in effects {
            execute_effect(effect, &mut app, &mut terminal, &mut guard)?;
        }
    }
    let _ = app.session_state().save();
    Ok(())
}

fn handle_app_event(app: &mut App, event: event::AppEvent, dirty: &mut bool) {
    match event {
        event::AppEvent::FsChanged => *dirty = true,
        event::AppEvent::GitFinished(result) => app.handle_git_task(result),
        event::AppEvent::GistFinished(result) => app.handle_gist_task(result),
    }
}

fn execute_effect(
    effect: Effect,
    app: &mut App,
    terminal: &mut TuiTerminal,
    guard: &mut TerminalGuard,
) -> Result<()> {
    match effect {
        Effect::SpawnEditor(request) => {
            guard.suspend()?;
            let outcome =
                editor::run_external_edit(&app.library, request, app.editor_cmd.as_deref());
            guard.resume()?;
            *terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
            match outcome {
                Ok(outcome) => app.handle_editor_outcome(outcome),
                Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
            }
        }
        Effect::ForceSave(request) => match editor::force_save(&app.library, &request) {
            Ok(()) => app.handle_editor_outcome(EditOutcome::Saved),
            Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
        },
        Effect::CopyToClipboard { text, label } => match crate::clipboard::copy(&text) {
            Ok(method) => {
                let method = match method {
                    ClipboardMethod::System => "system clipboard",
                    ClipboardMethod::Osc52 => "OSC 52",
                };
                app.set_status(
                    format!("copied {} B {label} ({method})", text.len()),
                    StatusLevel::Info,
                );
            }
            Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
        },
        Effect::OpenInVsCode { path } => {
            let vscode_bin = app.vscode_cmd.as_deref().unwrap_or("code");
            let parts = match shlex::split(vscode_bin).filter(|parts| !parts.is_empty()) {
                Some(parts) if !parts.is_empty() => parts,
                _ => {
                    app.set_status(
                        format!("invalid vscode command: {vscode_bin:?}"),
                        StatusLevel::Error,
                    );
                    return Ok(());
                }
            };
            let mut command = std::process::Command::new(&parts[0]);
            command.args(&parts[1..]);
            command.arg(&path);
            match command.spawn() {
                Ok(_) => {
                    let name = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("file");
                    app.set_status(format!("opened {name} in VS Code"), StatusLevel::Info);
                }
                Err(error) => {
                    app.set_status(
                        format!("cannot launch VS Code ({:?}): {error}", parts[0]),
                        StatusLevel::Error,
                    );
                }
            }
        }
        Effect::RunGit(action) => {
            // Only Init uses interactive mode (needs terminal for first-time
            // setup). All other actions are spawned as background tasks.
            debug_assert!(
                matches!(action, crate::git::GitAction::Init),
                "non-Init git actions should be spawned as background tasks"
            );
            guard.suspend()?;
            let outcome = crate::git::execute_interactive(app.library.root(), &action);
            if let Err(error) = &outcome {
                eprintln!("\nsnip: {error}\n\nPress any key to continue...");
                let _ = io::stderr().flush();
                wait_for_key();
            }
            guard.resume()?;
            *terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
            app.reprobe_git();
            match outcome {
                Ok(outcome) => app.set_status(outcome.message, StatusLevel::Info),
                Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
            }
            app.finish_git_operation();
        }
    }
    Ok(())
}

fn wait_for_key() {
    if enable_raw_mode().is_err() {
        return;
    }
    loop {
        match terminal_event::read() {
            Ok(Event::Key(_)) | Err(_) => break,
            Ok(_) => {}
        }
    }
    let _ = disable_raw_mode();
}

struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        setup_terminal()?;
        Ok(Self { active: true })
    }

    fn suspend(&mut self) -> Result<()> {
        if self.active {
            restore_terminal();
            self.active = false;
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if !self.active {
            setup_terminal()?;
            self.active = true;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            restore_terminal();
        }
    }
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture) {
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        return Err(error.into());
    }
    Ok(())
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
}

struct PanicHookGuard {
    previous: Arc<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync + 'static>,
}

impl PanicHookGuard {
    fn install() -> Self {
        let previous =
            Arc::<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync>::from(panic::take_hook());
        let hook = Arc::clone(&previous);
        panic::set_hook(Box::new(move |info| {
            restore_terminal();
            hook(info);
        }));
        Self { previous }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        let previous = Arc::clone(&self.previous);
        panic::set_hook(Box::new(move |info| previous(info)));
    }
}
