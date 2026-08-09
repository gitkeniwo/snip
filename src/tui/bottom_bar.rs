use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::app::App;
use super::key_labels;
use super::modal::Modal;
use super::selection::text_width;
use super::state::{Pane, StatusLevel};
use super::theme::TuiTheme;
use super::widgets;
use crate::keys::{Keymap, Mode};
use crate::tui::command::CommandId;

type Shortcut<'a> = (String, &'a str);
type ShortcutSet<'a> = &'a [Shortcut<'a>];

#[derive(Clone, Copy)]
struct ShortcutSpec {
    modes: &'static [Mode],
    commands: &'static [CommandId],
    action: &'static str,
    choice: ChordChoice,
    /// Display this key label verbatim instead of the effective chords. The
    /// pill still disappears when the command is unbound or shadowed.
    fixed_key: Option<&'static str>,
}

#[derive(Clone, Copy)]
enum ChordChoice {
    All,
    First,
    Last,
}

macro_rules! shortcut {
    ($action:literal; $modes:expr => $($command:ident),+ $(,)?) => {
        ShortcutSpec {
            modes: $modes,
            commands: &[$(CommandId::$command),+],
            action: $action,
            choice: ChordChoice::All,
            fixed_key: None,
        }
    };
}

/// Like [`shortcut!`], but the pill always reads `$key`, whatever the
/// effective chords are. Used where the canonical hint matters more than the
/// exact binding (e.g. `j/k` for navigation, since some fonts render the
/// arrow glyphs poorly).
macro_rules! shortcut_key {
    ($key:literal; $action:literal; $modes:expr => $($command:ident),+ $(,)?) => {{
        let mut spec = shortcut!($action; $modes => $($command),+);
        spec.fixed_key = Some($key);
        spec
    }};
}

macro_rules! shortcut_first {
    ($action:literal; $modes:expr => $($command:ident),+ $(,)?) => {{
        let mut spec = shortcut!($action; $modes => $($command),+);
        spec.choice = ChordChoice::First;
        spec
    }};
}

macro_rules! shortcut_last {
    ($action:literal; $modes:expr => $($command:ident),+ $(,)?) => {{
        let mut spec = shortcut!($action; $modes => $($command),+);
        spec.choice = ChordChoice::Last;
        spec
    }};
}

const GLOBAL: &[Mode] = &[Mode::Global];
const SIDEBAR: &[Mode] = &[Mode::Sidebar];
const LIST: &[Mode] = &[Mode::List];
const PREVIEW: &[Mode] = &[Mode::Preview];
const FRAGMENT: &[Mode] = &[Mode::Fragment];
const GRAB: &[Mode] = &[Mode::FragmentGrab];
const TRASH: &[Mode] = &[Mode::Trash];
const GIT: &[Mode] = &[Mode::Git];
const GIST: &[Mode] = &[Mode::Gist];

pub fn draw_bottom_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg)),
        area,
    );
    if let Some(modal) = &app.modal {
        match modal {
            Modal::Input(input) => {
                let prefix = format!("{}: ", input.label);
                let mut spans = vec![
                    Span::styled(
                        prefix.clone(),
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(input.value.clone()),
                ];
                if let Some(error) = &input.error {
                    spans.push(Span::styled("  ● ", Style::default().fg(app.theme.error)));
                    spans.push(Span::styled(
                        error.clone(),
                        Style::default().fg(app.theme.error),
                    ));
                }
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                let before_cursor = input.value.chars().take(input.cursor).count() as u16;
                let x = area
                    .x
                    .saturating_add(prefix.chars().count() as u16 + before_cursor)
                    .min(area.right().saturating_sub(1));
                frame.set_cursor_position((x, area.y));
            }
            Modal::Confirm(_) => frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "y/Enter",
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" confirm  ", Style::default().fg(app.theme.muted)),
                    Span::styled(
                        "n/Esc",
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" cancel", Style::default().fg(app.theme.muted)),
                ])),
                area,
            ),
            Modal::Picker(picker) => frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("/ ", Style::default().fg(app.theme.accent)),
                    Span::raw(picker.filter()),
                    Span::styled(
                        "  ↑/↓ move  Enter select  Esc cancel",
                        Style::default().fg(app.theme.muted),
                    ),
                ])),
                area,
            ),
        }
        return;
    }
    if app.search.active {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "/ ",
                    Style::default()
                        .fg(app.theme.warning)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(app.search.query.clone()),
            ])),
            area,
        );
        let x = area
            .x
            .saturating_add(2 + app.search.query.chars().count() as u16)
            .min(area.right().saturating_sub(1));
        frame.set_cursor_position((x, area.y));
        return;
    }
    if let Some(status) = &app.status {
        let color = match status.level {
            StatusLevel::Info => app.theme.success,
            StatusLevel::Error => app.theme.error,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(color)),
                Span::raw(status.text.clone()),
            ])),
            area,
        );
        return;
    }
    let mode_stack = Mode::stack(app);
    let navigation_full = resolve_shortcuts(
        &app.keymap,
        &mode_stack,
        &[
            shortcut_key!("j/k"; "nav"; GLOBAL => PaneBack, PaneForward),
            shortcut!("search"; GLOBAL => LibrarySearch),
            shortcut_first!("cmd"; GLOBAL => PaletteOpen),
            shortcut!("git"; GLOBAL => GitToggleConsole),
            shortcut!("help"; GLOBAL => ViewToggleHelp),
            shortcut!("quit"; GLOBAL => AppQuit),
        ],
    );
    let navigation_medium = resolve_shortcuts(
        &app.keymap,
        &mode_stack,
        &[
            shortcut_key!("j/k"; "nav"; GLOBAL => PaneBack, PaneForward),
            shortcut!("search"; GLOBAL => LibrarySearch),
            shortcut_first!("cmd"; GLOBAL => PaletteOpen),
            shortcut!("help"; GLOBAL => ViewToggleHelp),
        ],
    );
    let navigation_compact = resolve_shortcuts(
        &app.keymap,
        &mode_stack,
        &[
            shortcut_key!("j/k"; ""; GLOBAL => PaneBack, PaneForward),
            shortcut!(""; GLOBAL => LibrarySearch),
            shortcut_first!(""; GLOBAL => PaletteOpen),
            shortcut!(""; GLOBAL => ViewToggleHelp),
        ],
    );

    let (full_specs, medium_specs, compact_specs): (
        &[ShortcutSpec],
        &[ShortcutSpec],
        &[ShortcutSpec],
    ) = if app.git.open {
        (
            &[
                shortcut!("refresh"; GIT => GitRefreshLocalStatus),
                shortcut!("backup"; GIT => GitBackup),
                shortcut!("commit"; GIT => GitCommit),
                shortcut!("push"; GIT => GitPush),
                shortcut!("close"; GIT => UiDismiss),
            ],
            &[
                shortcut!("backup"; GIT => GitBackup),
                shortcut!("commit"; GIT => GitCommit),
                shortcut!("push"; GIT => GitPush),
                shortcut!("close"; GIT => UiDismiss),
            ],
            &[
                shortcut!(""; GIT => GitBackup),
                shortcut!(""; GIT => GitPush),
                shortcut!(""; GIT => UiDismiss),
            ],
        )
    } else if app.gist.open {
        (
            &[
                shortcut!("publish"; GIST => GistPush),
                shortcut!("copy"; GIST => GistCopyUrl),
                shortcut!("open"; GIST => GistOpenInBrowser),
                shortcut!("check"; GIST => GistVerifyRemote),
                shortcut!("close"; GIST => UiDismiss),
            ],
            &[
                shortcut!("publish"; GIST => GistPush),
                shortcut!("copy"; GIST => GistCopyUrl),
                shortcut!("open"; GIST => GistOpenInBrowser),
                shortcut!("close"; GIST => UiDismiss),
            ],
            &[
                shortcut!(""; GIST => GistPush),
                shortcut!(""; GIST => GistCopyUrl),
                shortcut!(""; GIST => UiDismiss),
            ],
        )
    } else if app.fragment_grab.is_some() {
        (
            &[
                shortcut_first!("move"; GRAB => NavDown, NavUp),
                shortcut!("drop"; GRAB => GrabDrop),
                shortcut_last!("cancel"; GRAB => UiDismiss),
            ],
            &[
                shortcut_first!("move"; GRAB => NavDown, NavUp),
                shortcut!("drop"; GRAB => GrabDrop),
                shortcut_last!("cancel"; GRAB => UiDismiss),
            ],
            &[
                shortcut!(""; GRAB => GrabDrop),
                shortcut_last!(""; GRAB => UiDismiss),
            ],
        )
    } else if app.fragment_context() {
        (
            &[
                shortcut!("add"; FRAGMENT => FragmentAdd),
                shortcut!("rename"; FRAGMENT => FragmentRename),
                shortcut!("reorder"; FRAGMENT => FragmentReorder),
                shortcut!("delete"; FRAGMENT => FragmentRemove),
                shortcut!("edit"; PREVIEW => SnippetEditContent),
                shortcut!("collapse"; PREVIEW => PreviewCollapseFragments),
            ],
            &[
                shortcut!("add"; FRAGMENT => FragmentAdd),
                shortcut!("rename"; FRAGMENT => FragmentRename),
                shortcut!("delete"; FRAGMENT => FragmentRemove),
                shortcut!("collapse"; PREVIEW => PreviewCollapseFragments),
            ],
            &[
                shortcut!(""; FRAGMENT => FragmentAdd),
                shortcut!(""; FRAGMENT => FragmentRename),
                shortcut!(""; FRAGMENT => FragmentRemove),
            ],
        )
    } else if app.trash.open && app.focus == Pane::List {
        (
            &[
                shortcut_first!("move"; TRASH => NavDown, NavUp),
                shortcut_first!("restore"; TRASH => TrashRestoreSelected),
                shortcut!("purge"; TRASH => TrashPurgeSelected),
            ],
            &[
                shortcut_first!("restore"; TRASH => TrashRestoreSelected),
                shortcut!("purge"; TRASH => TrashPurgeSelected),
            ],
            &[
                shortcut_first!(""; TRASH => TrashRestoreSelected),
                shortcut!(""; TRASH => TrashPurgeSelected),
            ],
        )
    } else {
        match app.focus {
            Pane::Sidebar => (
                &[
                    shortcut!("create"; SIDEBAR => FolderNew),
                    shortcut!("rename"; SIDEBAR => SidebarRename),
                    shortcut!("move"; SIDEBAR => FolderMove),
                    shortcut!("delete"; SIDEBAR => SidebarDelete),
                    shortcut!("sort"; GLOBAL => ViewCycleSort),
                ],
                &[
                    shortcut!("create"; SIDEBAR => FolderNew),
                    shortcut!("rename"; SIDEBAR => SidebarRename),
                    shortcut!("delete"; SIDEBAR => SidebarDelete),
                ],
                &[
                    shortcut!(""; SIDEBAR => FolderNew),
                    shortcut!(""; SIDEBAR => SidebarRename),
                    shortcut!(""; SIDEBAR => SidebarDelete),
                ],
            ),
            Pane::List | Pane::Preview => {
                let mode = if app.focus == Pane::List {
                    LIST
                } else {
                    PREVIEW
                };
                (
                    &[
                        shortcut!("create"; mode => SnippetNew),
                        shortcut!("edit"; mode => SnippetEditContent),
                        shortcut!("tags"; mode => SnippetEditTags),
                        shortcut!("rename"; mode => SnippetRename),
                        shortcut!("move"; mode => SnippetMove),
                        shortcut!("copy"; GLOBAL => CopyContent),
                        shortcut!("path"; GLOBAL => CopyManagedPath),
                    ],
                    &[
                        shortcut!("create"; mode => SnippetNew),
                        shortcut!("edit"; mode => SnippetEditContent),
                        shortcut!("tags"; mode => SnippetEditTags),
                        shortcut!("copy"; GLOBAL => CopyContent),
                    ],
                    &[
                        shortcut!(""; mode => SnippetNew),
                        shortcut!(""; mode => SnippetEditContent),
                        shortcut!(""; GLOBAL => CopyContent),
                    ],
                )
            }
        }
    };
    let actions_full = resolve_shortcuts(&app.keymap, &mode_stack, full_specs);
    let actions_medium = resolve_shortcuts(&app.keymap, &mode_stack, medium_specs);
    let actions_compact = resolve_shortcuts(&app.keymap, &mode_stack, compact_specs);

    let tiers = [
        (navigation_full.as_slice(), actions_full.as_slice()),
        (navigation_medium.as_slice(), actions_medium.as_slice()),
        (navigation_compact.as_slice(), actions_medium.as_slice()),
        (navigation_compact.as_slice(), actions_compact.as_slice()),
        (
            navigation_compact.as_slice(),
            &actions_compact[..actions_compact.len().min(1)],
        ),
    ];
    let (navigation, actions) = tiers
        .into_iter()
        .find(|(navigation, actions)| {
            shortcut_pills_width(navigation) + shortcut_pills_width(actions) + 2
                <= area.width as usize
        })
        .unwrap_or((
            navigation_compact.as_slice(),
            &actions_compact[..actions_compact.len().min(1)],
        ));

    let left = widgets::square_start(shortcut_pills(navigation, app.theme));
    let right = widgets::square_end(shortcut_pills_with_primary(
        actions,
        app.theme,
        if app.git.open
            || app.gist.open
            || app.trash.open
            || app.fragment_grab.is_some()
            || app.fragment_context()
        {
            app.theme.accent_alt
        } else {
            app.theme.pill_primary
        },
    ));
    let right_width = right.width().min(area.width as usize) as u16;
    let regions =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);
    frame.render_widget(Paragraph::new(left), regions[0]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        regions[1],
    );
}

fn resolve_shortcuts(
    keymap: &Keymap,
    stack: &[Mode],
    specs: &[ShortcutSpec],
) -> Vec<Shortcut<'static>> {
    specs
        .iter()
        .filter_map(|spec| {
            let mut chords = Vec::new();
            for command in spec.commands {
                let bindings = spec
                    .modes
                    .iter()
                    .map(|mode| (*mode, *command))
                    .collect::<Vec<_>>();
                let command_chords = key_labels::effective_chords(keymap, stack, &bindings);
                match spec.choice {
                    ChordChoice::All => chords.extend(command_chords),
                    ChordChoice::First => chords.extend(command_chords.into_iter().next()),
                    ChordChoice::Last => chords.extend(command_chords.into_iter().next_back()),
                }
            }
            chords.dedup();
            (!chords.is_empty()).then(|| {
                let key = match spec.fixed_key {
                    Some(label) => label.to_owned(),
                    None => chords
                        .into_iter()
                        .map(key_labels::compact_chord)
                        .collect::<Vec<_>>()
                        .join("/"),
                };
                (key, spec.action)
            })
        })
        .collect()
}

fn shortcut_pills_width(commands: ShortcutSet<'_>) -> usize {
    commands
        .iter()
        .map(|(key, action)| {
            2 + text_width(key) as usize
                + if action.is_empty() {
                    0
                } else {
                    2 + text_width(action) as usize
                }
        })
        .sum::<usize>()
        + commands.len().saturating_sub(1)
}

fn shortcut_pills(commands: ShortcutSet<'_>, theme: TuiTheme) -> Line<'static> {
    shortcut_pills_with_primary(commands, theme, theme.pill_primary)
}

fn shortcut_pills_with_primary(
    commands: ShortcutSet<'_>,
    theme: TuiTheme,
    primary: ratatui::style::Color,
) -> Line<'static> {
    let secondary = theme.pill_secondary;
    let mut spans = Vec::new();
    for (index, (key, action)) in commands.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" ", Style::default().bg(theme.bar_bg)));
        }
        spans.push(widgets::pill_cap(widgets::PILL_OPEN, primary, theme.bar_bg));
        spans.push(Span::styled(
            key.clone(),
            Style::default()
                .fg(theme.legible_on(primary, theme.selection_fg))
                .bg(primary)
                .add_modifier(Modifier::BOLD),
        ));
        if action.is_empty() {
            spans.push(widgets::pill_cap(
                widgets::PILL_CLOSE,
                primary,
                theme.bar_bg,
            ));
        } else {
            spans.push(widgets::pill_cap(widgets::PILL_CLOSE, primary, secondary));
            spans.push(Span::styled(
                format!(" {action}"),
                Style::default()
                    .fg(theme.legible_on(secondary, theme.bar_fg))
                    .bg(secondary)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(widgets::pill_cap(
                widgets::PILL_CLOSE,
                secondary,
                theme.bar_bg,
            ));
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_specs_use_compact_effective_bindings() {
        let keymap = Keymap::defaults();
        let shortcuts = resolve_shortcuts(
            &keymap,
            &[Mode::List, Mode::Global],
            &[
                shortcut_key!("j/k"; "nav"; GLOBAL => PaneBack, PaneForward),
                shortcut!("search"; GLOBAL => LibrarySearch),
                shortcut_first!("cmd"; GLOBAL => PaletteOpen),
                shortcut!("git"; GLOBAL => GitToggleConsole),
            ],
        );

        assert_eq!(shortcuts[0], ("j/k".to_owned(), "nav"));
        assert_eq!(shortcuts[1], ("/".to_owned(), "search"));
        assert_eq!(shortcuts[2], (":".to_owned(), "cmd"));
        assert_eq!(shortcuts[3], ("^g".to_owned(), "git"));
    }

    #[test]
    fn secondary_shortcut_labels_prefer_neutral_bar_foreground() {
        let theme = TuiTheme::default_for(crate::theme::Appearance::Light);
        let commands = [("n".to_owned(), "create")];
        let line = shortcut_pills(&commands, theme);
        let action = line
            .spans
            .iter()
            .find(|span| span.content == " create")
            .unwrap();

        assert_eq!(action.style.fg, Some(theme.bar_fg));
        assert_eq!(action.style.bg, Some(theme.pill_secondary));
    }

    #[test]
    fn unbound_shortcuts_disappear_and_user_bindings_replace_defaults() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(
            &path,
            "[global]\n\"git.toggle-console\" = \"alt-g\"\n\"view.toggle-help\" = []\n",
        )
        .unwrap();
        let (keymap, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());

        let shortcuts = resolve_shortcuts(
            &keymap,
            &[Mode::List, Mode::Global],
            &[
                shortcut!("git"; GLOBAL => GitToggleConsole),
                shortcut!("help"; GLOBAL => ViewToggleHelp),
            ],
        );
        assert_eq!(shortcuts, [("Alt-g".to_owned(), "git")]);
    }

    #[test]
    fn exclusive_modes_hide_shadowed_global_shortcuts() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(&path, "[global]\n\"app.quit\" = \"x\"\n").unwrap();
        let (keymap, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());

        let shortcuts = resolve_shortcuts(
            &keymap,
            &[Mode::Trash],
            &[
                shortcut!("quit"; GLOBAL => AppQuit),
                shortcut!("purge"; TRASH => TrashPurgeSelected),
            ],
        );
        assert_eq!(shortcuts, [("x".to_owned(), "purge")]);
    }

    #[test]
    fn panel_shortcuts_use_their_own_mode_bindings() {
        let keymap = Keymap::defaults();

        let git = resolve_shortcuts(
            &keymap,
            &[Mode::Git],
            &[
                shortcut!("backup"; GIT => GitBackup),
                shortcut!("commit"; GIT => GitCommit),
                shortcut!("push"; GIT => GitPush),
                shortcut!("close"; GIT => UiDismiss),
            ],
        );
        assert_eq!(
            git,
            [
                ("b".to_owned(), "backup"),
                ("c".to_owned(), "commit"),
                ("p".to_owned(), "push"),
                ("Esc".to_owned(), "close"),
            ]
        );

        let gist = resolve_shortcuts(
            &keymap,
            &[Mode::Gist],
            &[
                shortcut!("publish"; GIST => GistPush),
                shortcut!("copy"; GIST => GistCopyUrl),
                shortcut!("open"; GIST => GistOpenInBrowser),
                shortcut!("close"; GIST => UiDismiss),
            ],
        );
        assert_eq!(
            gist,
            [
                ("p".to_owned(), "publish"),
                ("y".to_owned(), "copy"),
                ("o".to_owned(), "open"),
                ("Esc".to_owned(), "close"),
            ]
        );
    }
}
