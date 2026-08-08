use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

use super::app::App;
use super::command::CommandId;
use super::key_labels;
use super::selection::text_width;
use super::theme::TuiTheme;
use super::widgets;
use crate::keys::{Keymap, Mode};

#[derive(Clone, Copy)]
struct BindingRef {
    modes: &'static [Mode],
    command: CommandId,
}

#[derive(Clone, Copy)]
enum EntryKeys {
    Bindings(&'static [BindingRef]),
    Fixed(&'static str),
}

#[derive(Clone, Copy)]
struct Entry {
    keys: EntryKeys,
    description: &'static str,
}

macro_rules! keys {
    ($description:literal; $($modes:expr => $command:ident),+ $(,)?) => {
        Entry {
            keys: EntryKeys::Bindings(&[
                $(BindingRef { modes: $modes, command: CommandId::$command }),+
            ]),
            description: $description,
        }
    };
}

macro_rules! fixed {
    ($label:literal, $description:literal) => {
        Entry {
            keys: EntryKeys::Fixed($label),
            description: $description,
        }
    };
}

const HELP_KEY_WIDTH: usize = 15;
const GLOBAL: &[Mode] = &[Mode::Global];
const SIDEBAR: &[Mode] = &[Mode::Sidebar];
const LIST: &[Mode] = &[Mode::List];
const PREVIEW: &[Mode] = &[Mode::Preview];
const FRAGMENT: &[Mode] = &[Mode::Fragment];
const GRAB: &[Mode] = &[Mode::FragmentGrab];
const TRASH: &[Mode] = &[Mode::Trash];
const GIT: &[Mode] = &[Mode::Git];
const GIST: &[Mode] = &[Mode::Gist];
const LIST_PREVIEW: &[Mode] = &[Mode::List, Mode::Preview];
const NAVIGATION: &[Mode] = &[
    Mode::Sidebar,
    Mode::List,
    Mode::Preview,
    Mode::FragmentGrab,
    Mode::Trash,
    Mode::Help,
];
const CONFIGURABLE: &[Mode] = &Mode::CONFIGURABLE;

const GROUPS: &[(&str, &[Entry], HelpColor)] = &[
    (
        "MOVE — ALL PANES",
        &[
            keys!("next / previous pane"; GLOBAL => PaneNext, GLOBAL => PanePrevious),
            keys!("back / drill in"; GLOBAL => PaneBack, GLOBAL => PaneForward),
            keys!("next / previous item"; NAVIGATION => NavDown, NAVIGATION => NavUp),
            keys!("first / last item"; NAVIGATION => NavFirst, NAVIGATION => NavLast),
            fixed!("1-9 / 0", "jump to 1st-10th item"),
            keys!("page down / up"; NAVIGATION => NavPageDown, NAVIGATION => NavPageUp),
            keys!("previous / next fragment"; GLOBAL => PreviewPreviousItem, GLOBAL => PreviewNextItem),
            keys!("previous / next paragraph"; GLOBAL => PreviewPreviousParagraph, GLOBAL => PreviewNextParagraph),
        ],
        HelpColor::Accent,
    ),
    (
        "SIDEBAR — WHEN THE LEFT PANE HAS FOCUS",
        &[
            keys!("expand / collapse folder"; SIDEBAR => SidebarToggleFolder),
            keys!("apply selected filter"; SIDEBAR => SidebarActivate),
            keys!("create folder"; SIDEBAR => FolderNew),
            keys!("rename folder or tag"; SIDEBAR => SidebarRename, CONFIGURABLE => FolderRename, CONFIGURABLE => TagRename),
            keys!("move folder"; SIDEBAR => FolderMove),
            keys!("delete folder or tag"; SIDEBAR => SidebarDelete, CONFIGURABLE => FolderDelete, CONFIGURABLE => TagDelete),
        ],
        HelpColor::Tag,
    ),
    (
        "SNIPPETS — WHEN LIST OR PREVIEW HAS FOCUS",
        &[
            keys!("enter preview"; LIST => ListEnterPreview),
            keys!("create snippet"; LIST_PREVIEW => SnippetNew),
            keys!("edit content"; LIST_PREVIEW => SnippetEditContent),
            keys!("edit note"; LIST_PREVIEW => SnippetEditNote),
            keys!("edit README"; LIST_PREVIEW => SnippetEditReadme),
            keys!("open in VS Code"; LIST_PREVIEW => SnippetOpenVsCode),
            keys!("rename snippet"; LIST_PREVIEW => SnippetRename),
            keys!("move snippet"; LIST_PREVIEW => SnippetMove),
            keys!("edit tags"; LIST_PREVIEW => SnippetEditTags),
            keys!("edit language"; LIST_PREVIEW => SnippetEditLanguage),
            keys!("toggle pin"; LIST_PREVIEW => SnippetTogglePin),
            keys!("toggle lock"; LIST_PREVIEW => SnippetToggleLock),
            keys!("move to trash"; LIST_PREVIEW => SnippetMoveToTrash),
        ],
        HelpColor::Alt,
    ),
    (
        "FRAGMENTS — WHEN THE LIST IS EXPANDED AND PREVIEW HAS FOCUS",
        &[
            keys!("expand / collapse the list"; PREVIEW => PreviewExpandFragments, PREVIEW => PreviewCollapseFragments),
            keys!("add fragment"; FRAGMENT => FragmentAdd),
            keys!("rename fragment"; FRAGMENT => FragmentRename),
            keys!("reorder fragment"; FRAGMENT => FragmentReorder),
            keys!("delete fragment"; FRAGMENT => FragmentRemove),
            keys!("move a grabbed fragment"; GRAB => NavDown, GRAB => NavUp),
            keys!("drop it"; GRAB => GrabDrop),
            keys!("cancel the move"; GRAB => UiDismiss),
        ],
        HelpColor::Warning,
    ),
    (
        "COPY",
        &[
            keys!("content"; GLOBAL => CopyContent),
            keys!("snippet ID"; GLOBAL => CopySnippetId),
            keys!("managed path"; GLOBAL => CopyManagedPath),
        ],
        HelpColor::Success,
    ),
    (
        "VIEW & GLOBAL",
        &[
            keys!("search"; GLOBAL => LibrarySearch),
            keys!("open command palette"; GLOBAL => PaletteOpen),
            keys!("cycle sort"; GLOBAL => ViewCycleSort),
            keys!("sort by modified"; CONFIGURABLE => ViewSortModified),
            keys!("sort by title"; CONFIGURABLE => ViewSortTitle),
            keys!("sort by created"; CONFIGURABLE => ViewSortCreated),
            keys!("toggle line numbers"; GLOBAL => ViewToggleLineNumbers),
            keys!("toggle fragment list"; CONFIGURABLE => ViewToggleFragmentList),
            keys!("toggle list density"; GLOBAL => ViewToggleDensity),
            keys!("toggle light / dark"; GLOBAL => ViewCycleAppearance),
            keys!("clear appearance override"; CONFIGURABLE => ViewFollowSystemAppearance),
            keys!("change color theme"; CONFIGURABLE => ViewPickTheme),
            keys!("toggle trash"; GLOBAL => LibraryToggleTrash),
            keys!("clear filter"; CONFIGURABLE => LibraryClearFilter),
            keys!("toggle published filter"; CONFIGURABLE => LibraryTogglePublishedFilter),
            keys!("Git console"; GLOBAL => GitToggleConsole),
            keys!("Gist panel"; GLOBAL => GistTogglePanel),
            keys!("rescan library"; GLOBAL => LibraryRescan),
            keys!("toggle help"; GLOBAL => ViewToggleHelp),
            keys!("close or clear"; GLOBAL => UiDismiss),
            keys!("quit"; GLOBAL => AppQuit),
            fixed!("Ctrl-c", "force quit"),
        ],
        HelpColor::Warning,
    ),
    (
        "MOUSE",
        &[
            fixed!("wheel", "scroll hovered pane"),
            fixed!("click", "select item or fragment"),
            fixed!("double-click", "drill into preview"),
            fixed!("drag", "select preview text"),
            fixed!("mouse up", "copy selection"),
        ],
        HelpColor::Success,
    ),
    (
        "TRASH — WHEN THE TRASH PANE HAS FOCUS",
        &[
            keys!("move"; TRASH => NavDown, TRASH => NavUp),
            keys!("restore"; TRASH => TrashRestoreSelected),
            keys!("purge permanently"; TRASH => TrashPurgeSelected),
            keys!("leave the trash"; TRASH => UiDismiss),
        ],
        HelpColor::Error,
    ),
    (
        "GIT CONSOLE — WHEN OPEN",
        &[
            keys!("backup"; GIT => GitBackup),
            keys!("commit"; GIT => GitCommit),
            keys!("push"; GIT => GitPush),
            keys!("fetch remote status"; GIT => GitFetchRemoteStatus),
            keys!("pull from remote"; GIT => GitPull),
            keys!("custom commit message"; GIT => GitCommitWithMessage),
            keys!("pause this session"; GIT => GitPauseAutoBackup),
            keys!("initialize repository, or set automatic interval"; GIT => GitInitOrSetInterval, CONFIGURABLE => GitInitRepository, CONFIGURABLE => GitSetAutoCommitInterval),
            keys!("toggle automatic push"; GIT => GitToggleAutoPush),
            keys!("toggle automatic pull on start"; GIT => GitToggleAutoPull),
            keys!("toggle backup on quit"; GIT => GitToggleBackupOnQuit),
            keys!("refresh local status"; GIT => GitRefreshLocalStatus),
            keys!("close console"; GIT => UiDismiss),
        ],
        HelpColor::Alt,
    ),
    (
        "GIST PANEL — WHEN OPEN",
        &[
            keys!("publish or update"; GIST => GistPush),
            keys!("publish as public"; GIST => GistPushPublic),
            keys!("copy link"; GIST => GistCopyUrl),
            keys!("open in browser"; GIST => GistOpenInBrowser),
            keys!("link an existing gist"; GIST => GistAttach),
            keys!("check it still exists"; GIST => GistVerifyRemote),
            keys!("unlink"; GIST => GistDetach),
            keys!("delete on GitHub"; GIST => GistDelete),
            keys!("close panel"; GIST => UiDismiss),
        ],
        HelpColor::Success,
    ),
];

#[derive(Clone, Copy)]
enum HelpColor {
    Accent,
    Alt,
    Tag,
    Success,
    Warning,
    Error,
}

pub fn draw_help(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup_width = area.width.saturating_sub(4).min(120);
    // Two border cells plus three cells of padding on each side.
    let content_width = popup_width.saturating_sub(8) as usize;
    let columns = if content_width >= 108 {
        3
    } else if content_width >= 68 {
        2
    } else {
        1
    };
    let content = help_content(columns, content_width, app.theme, &app.keymap);
    let desired_height = u16::try_from(content.lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(7);
    let popup = widgets::centered_rect(
        popup_width,
        desired_height.min(area.height.saturating_sub(2)),
        area,
    );
    frame.render_widget(Clear, popup);
    super::widgets::fill_surface(frame, popup, app.theme);
    let block = Block::default()
        .title(Line::from(" Help ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .padding(Padding::new(3, 3, 1, 1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "snip TUI",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .centered(),
            Line::styled(
                "keys are grouped by context · * marks a user binding",
                Style::default().fg(app.theme.muted),
            )
            .centered(),
        ])),
        rows[0],
    );
    let max_scroll = content
        .lines
        .len()
        .saturating_sub(rows[1].height as usize)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(content).scroll((app.help_scroll.min(max_scroll), 0)),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(help_footer(app.theme, &app.keymap)).alignment(Alignment::Center),
        rows[2],
    );
}

fn help_footer(theme: TuiTheme, keymap: &Keymap) -> Line<'static> {
    let stack = [Mode::Help];
    let scroll = key_labels::display_primary_bindings(
        keymap,
        &stack,
        &[
            (Mode::Help, CommandId::NavDown),
            (Mode::Help, CommandId::NavUp),
            (Mode::Help, CommandId::NavPageDown),
            (Mode::Help, CommandId::NavPageUp),
        ],
    );
    let close = key_labels::display_primary_bindings(
        keymap,
        &stack,
        &[
            (Mode::Help, CommandId::UiDismiss),
            (Mode::Global, CommandId::ViewToggleHelp),
        ],
    );
    let scroll = if scroll.is_empty() {
        "wheel".to_owned()
    } else {
        format!("wheel · {scroll}")
    };
    let mut spans = vec![
        Span::styled(
            scroll,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  scroll", Style::default().fg(theme.muted)),
    ];
    if !close.is_empty() {
        spans.push(Span::styled("    ", Style::default().fg(theme.muted)));
        spans.push(Span::styled(
            close,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            "  close help",
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}

fn help_content(columns: usize, width: usize, theme: TuiTheme, keymap: &Keymap) -> Text<'static> {
    let defaults = Keymap::defaults();
    let mut lines = Vec::new();
    for (label, entries, color) in GROUPS {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.extend(help_panel(
            label,
            entries,
            columns,
            width,
            resolve_color(*color, theme),
            theme,
            (keymap, &defaults),
        ));
    }
    Text::from(lines)
}

fn help_panel(
    label: &str,
    entries: &[Entry],
    columns: usize,
    width: usize,
    key_color: Color,
    theme: TuiTheme,
    keymaps: (&Keymap, &Keymap),
) -> Vec<Line<'static>> {
    let (keymap, defaults) = keymaps;
    let mut lines = vec![
        Line::from(vec![
            Span::styled("── ", Style::default().fg(theme.rule)),
            Span::styled(
                label.to_owned(),
                Style::default().fg(key_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ──", Style::default().fg(theme.rule)),
        ])
        .centered(),
    ];
    let rows = entries.len().div_ceil(columns);
    let column_width = width / columns;
    let extra_columns = width % columns;
    for row in 0..rows {
        let mut spans = Vec::new();
        for column in 0..columns {
            let index = row + column * rows;
            let entry = entries.get(index).copied();
            let keys = entry
                .map(|entry| entry_keys(entry, keymap, defaults))
                .unwrap_or_default();
            let description = entry.map(|entry| entry.description).unwrap_or_default();
            let cell_width = column_width + usize::from(column < extra_columns);
            let key_width = HELP_KEY_WIDTH.min(cell_width.saturating_sub(2));
            let description_width = cell_width.saturating_sub(key_width + 2);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                pad_display(&keys, key_width),
                Style::default().fg(key_color).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                pad_display(description, description_width),
                Style::default().fg(theme.muted),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn entry_keys(entry: Entry, keymap: &Keymap, defaults: &Keymap) -> String {
    let EntryKeys::Bindings(bindings) = entry.keys else {
        return match entry.keys {
            EntryKeys::Fixed(label) => label.to_owned(),
            EntryKeys::Bindings(_) => unreachable!(),
        };
    };

    let mut labels = Vec::new();
    let mut user_modified = false;
    for binding in bindings {
        let chords = keymap.chords_for(binding.modes, binding.command);
        user_modified |= binding.modes.iter().any(|mode| {
            keymap.chords_for(&[*mode], binding.command)
                != defaults.chords_for(&[*mode], binding.command)
        });
        if chords.is_empty() {
            continue;
        }
        let label = chords
            .into_iter()
            .map(help_chord)
            .collect::<Vec<_>>()
            .join(" / ");
        if !labels.contains(&label) {
            labels.push(label);
        }
    }

    let mut label = if labels.is_empty() {
        "—".to_owned()
    } else {
        labels.join(" / ")
    };
    if user_modified {
        label.push_str(" *");
    }
    label
}

fn help_chord(chord: crate::keys::Chord) -> String {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    if chord.modifiers() == KeyModifiers::NONE {
        match chord.code() {
            KeyCode::Up => return "↑".to_owned(),
            KeyCode::Down => return "↓".to_owned(),
            KeyCode::Left => return "←".to_owned(),
            KeyCode::Right => return "→".to_owned(),
            _ => {}
        }
    }
    chord.display()
}

fn resolve_color(color: HelpColor, theme: TuiTheme) -> Color {
    match color {
        HelpColor::Accent => theme.accent,
        HelpColor::Alt => theme.accent_alt,
        HelpColor::Tag => theme.tag,
        HelpColor::Success => theme.success,
        HelpColor::Warning => theme.warning,
        HelpColor::Error => theme.error,
    }
}

fn pad_display(value: &str, width: usize) -> String {
    let value = widgets::truncate_end(value, width);
    let used = text_width(&value) as usize;
    format!("{value}{}", " ".repeat(width.saturating_sub(used)))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    #[test]
    fn help_documents_every_default_action() {
        let keymap = Keymap::defaults();
        let bound = Mode::ALL
            .into_iter()
            .flat_map(|mode| keymap.bindings_for(mode).map(|(_, command)| command))
            .collect::<HashSet<_>>();
        let documented = GROUPS
            .iter()
            .flat_map(|(_, entries, _)| *entries)
            .flat_map(|entry| match entry.keys {
                EntryKeys::Bindings(bindings) => bindings,
                EntryKeys::Fixed(_) => &[],
            })
            .map(|binding| binding.command)
            .collect::<HashSet<_>>();

        assert_eq!(
            bound.difference(&documented).collect::<Vec<_>>(),
            Vec::<&CommandId>::new(),
            "every default action must have an owning help entry"
        );
    }

    #[test]
    fn help_cells_preserve_keys_and_explicitly_ellipsize_descriptions() {
        let theme = TuiTheme::default_for(super::super::theme::Appearance::Dark);
        let keymap = Keymap::defaults();
        let lines = help_panel(
            GROUPS[0].0,
            GROUPS[0].1,
            3,
            108,
            resolve_color(GROUPS[0].2, theme),
            theme,
            (&keymap, &keymap),
        );
        assert!(lines.iter().all(|line| line.width() <= 108));
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(rendered.contains("Tab / Shift-Tab"));
        assert!(rendered.contains("Ctrl-d / Ctrl-u"));
        assert!(
            rendered.contains('…'),
            "a clipped description must advertise truncation"
        );
    }

    #[test]
    fn every_palette_command_appears_in_exactly_one_help_group() {
        let mut groups_per_command = HashMap::<CommandId, usize>::new();
        for (_, entries, _) in GROUPS {
            let commands = entries
                .iter()
                .flat_map(|entry| match entry.keys {
                    EntryKeys::Bindings(bindings) => bindings,
                    EntryKeys::Fixed(_) => &[],
                })
                .map(|binding| binding.command)
                .filter(|id| crate::tui::command::get(*id).palette)
                .collect::<HashSet<_>>();
            for command in commands {
                *groups_per_command.entry(command).or_default() += 1;
            }
        }

        for command in crate::tui::command::registry() {
            if !command.palette {
                continue;
            }
            assert_eq!(
                groups_per_command.get(&command.id),
                Some(&1),
                "{} must appear in exactly one help group",
                command.slug
            );
        }
    }

    #[test]
    fn defaults_have_no_intra_mode_collisions() {
        let defaults = Keymap::defaults();
        for mode in Mode::ALL {
            let chords = defaults
                .bindings_for(mode)
                .map(|(chord, _)| chord)
                .collect::<Vec<_>>();
            assert_eq!(
                chords.iter().copied().collect::<HashSet<_>>().len(),
                chords.len(),
                "{mode:?} contains a duplicate default chord"
            );
        }
    }

    #[test]
    fn user_modified_help_entries_are_marked() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(&path, "[list]\n\"snippet.edit-content\" = []\n").unwrap();
        let (keymap, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());
        let defaults = Keymap::defaults();
        let entry = GROUPS
            .iter()
            .flat_map(|(_, entries, _)| *entries)
            .find(|entry| entry.description == "edit content")
            .unwrap();

        let label = entry_keys(*entry, &keymap, &defaults);
        assert_eq!(label, "e *");
        assert!(label.ends_with(" *"));
    }

    #[test]
    fn help_footer_uses_effective_help_bindings() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(
            &path,
            r#"
                [global]
                "view.toggle-help" = []

                [help]
                "nav.down" = "n"
                "nav.up" = "p"
                "nav.page-down" = []
                "nav.page-up" = []
                "ui.dismiss" = "x"
            "#,
        )
        .unwrap();
        let (keymap, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());
        let footer = help_footer(
            TuiTheme::default_for(super::super::theme::Appearance::Dark),
            &keymap,
        )
        .spans
        .into_iter()
        .map(|span| span.content.into_owned())
        .collect::<String>();

        assert!(footer.contains("wheel · n / p"));
        assert!(footer.contains("x  close help"));
        for stale in ["j/k", "Ctrl-d", "Ctrl-u", "Esc"] {
            assert!(!footer.contains(stale));
        }
    }
}
