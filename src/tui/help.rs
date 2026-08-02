use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

use super::app::App;
use super::selection::text_width;
use super::theme::TuiTheme;
use super::widgets;

type Entry = (&'static str, &'static str);
const HELP_KEY_WIDTH: usize = 15;

const GROUPS: &[(&str, &[Entry], HelpColor)] = &[
    (
        "MOVE — ALL PANES",
        &[
            ("Tab / Shift-Tab", "next / previous pane"),
            ("h / ←   l / →", "back / drill in"),
            ("j / ↓   k / ↑", "next / previous item"),
            ("g   G", "first / last item"),
            ("1-9 / 0", "jump to 1st-10th item"),
            ("Ctrl-d / Ctrl-u", "page down / up"),
            ("[   ]", "previous / next fragment"),
            ("{   }", "previous / next paragraph"),
        ],
        HelpColor::Accent,
    ),
    (
        "SIDEBAR — WHEN THE LEFT PANE HAS FOCUS",
        &[
            ("Space", "expand / collapse folder"),
            ("Enter", "apply selected filter"),
            ("n", "create folder"),
            ("r", "rename folder or tag"),
            ("m", "move folder"),
            ("d", "delete folder or tag"),
        ],
        HelpColor::Tag,
    ),
    (
        "SNIPPETS — WHEN LIST OR PREVIEW HAS FOCUS",
        &[
            ("Enter", "enter preview"),
            ("n", "create snippet"),
            ("e", "edit content"),
            ("E", "edit note"),
            ("R", "edit README"),
            ("v", "open in VS Code"),
            ("r", "rename snippet"),
            ("m", "move snippet"),
            ("t", "edit tags"),
            ("f", "edit language"),
            ("P", "toggle pin"),
            ("L", "toggle lock"),
            ("d", "move to trash"),
        ],
        HelpColor::Alt,
    ),
    (
        "COPY",
        &[("y", "content"), ("Y", "snippet ID"), ("p", "managed path")],
        HelpColor::Success,
    ),
    (
        "VIEW & GLOBAL",
        &[
            ("/", "search"),
            (": / Ctrl-P", "open command palette"),
            ("s", "cycle sort"),
            ("N", "toggle line numbers"),
            ("= / -", "expand / collapse fragments"),
            ("z", "toggle list density"),
            ("T", "open trash"),
            ("Ctrl-g", "Git console"),
            ("Ctrl-s", "Gist panel"),
            ("F5 / Ctrl-r", "rescan library"),
            ("?", "toggle help"),
            ("Esc", "close or clear"),
            ("q", "quit"),
            ("Ctrl-c", "force quit"),
        ],
        HelpColor::Warning,
    ),
    (
        "MOUSE",
        &[
            ("wheel", "scroll hovered pane"),
            ("click", "select item or fragment"),
            ("double-click", "drill into preview"),
            ("drag", "select preview text"),
            ("mouse up", "copy selection"),
        ],
        HelpColor::Success,
    ),
    (
        "TRASH — WHEN OPEN",
        &[
            ("j / k", "move"),
            ("u", "restore"),
            ("x", "purge permanently"),
        ],
        HelpColor::Error,
    ),
    (
        "GIT CONSOLE — WHEN OPEN",
        &[
            ("b", "backup"),
            ("c", "commit"),
            ("p", "push"),
            ("f", "fetch remote status"),
            ("C", "custom commit message"),
            ("a", "pause this session"),
            ("i", "initialize repository, or set automatic interval"),
            ("u", "toggle automatic push"),
            ("o", "toggle backup on quit"),
            ("r", "refresh local status"),
            ("Esc / Ctrl-g", "close console"),
        ],
        HelpColor::Alt,
    ),
    (
        "GIST PANEL — WHEN OPEN",
        &[
            ("p", "publish or update"),
            ("P", "publish as public"),
            ("y", "copy link"),
            ("o", "open in browser"),
            ("a", "link an existing gist"),
            ("r", "check it still exists"),
            ("d", "unlink"),
            ("x", "delete on GitHub"),
            ("Esc / Ctrl-s", "close panel"),
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
    let content = help_content(columns, content_width, app.theme);
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
                "keys are grouped by the pane or console that owns them",
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
        Paragraph::new(Line::from(vec![
            Span::styled(
                "wheel · j/k · Ctrl-d/u",
                Style::default()
                    .fg(app.theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  scroll    ", Style::default().fg(app.theme.muted)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(app.theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  close help", Style::default().fg(app.theme.muted)),
        ]))
        .alignment(Alignment::Center),
        rows[2],
    );
}

fn help_content(columns: usize, width: usize, theme: TuiTheme) -> Text<'static> {
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
) -> Vec<Line<'static>> {
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
            let entry = entries.get(index).copied().unwrap_or(("", ""));
            let cell_width = column_width + usize::from(column < extra_columns);
            let key_width = HELP_KEY_WIDTH.min(cell_width.saturating_sub(2));
            let description_width = cell_width.saturating_sub(key_width + 2);
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                pad_display(entry.0, key_width),
                Style::default().fg(key_color).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                pad_display(entry.1, description_width),
                Style::default().fg(theme.muted),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines
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
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn help_documents_every_literal_character_binding_in_input_routing() {
        let mut bound = BTreeSet::new();
        for source in [
            include_str!("app/input/mod.rs"),
            include_str!("app/input/gist.rs"),
            include_str!("app/input/git.rs"),
            include_str!("app/input/overlay.rs"),
            include_str!("app/input/panes.rs"),
            include_str!("app/trash_view.rs"),
        ] {
            for tail in source.split("KeyCode::Char('").skip(1) {
                if let Some(character) = tail.chars().next() {
                    bound.insert(character);
                }
            }
        }
        let mut documented = BTreeSet::new();
        for (group, entries, _) in GROUPS {
            for (label, description) in *entries {
                assert!(
                    !group.is_empty() && !description.is_empty(),
                    "every documented key needs an owning context and action"
                );
                if *label == "Space" {
                    documented.insert(' ');
                }
                if *label == "/" {
                    documented.insert('/');
                }
                for token in label.split_whitespace() {
                    if token == "/" {
                        continue;
                    }
                    if token == "1-9" {
                        for character in '1'..='9' {
                            documented.insert(character);
                        }
                        continue;
                    }
                    let mut characters = token.chars();
                    if let (Some(character), None) = (characters.next(), characters.next())
                        && character.is_ascii()
                    {
                        documented.insert(character);
                    }
                }
            }
        }
        assert_eq!(
            documented, bound,
            "Help and literal character routing must match as exact key tokens"
        );
    }

    #[test]
    fn help_cells_preserve_keys_and_explicitly_ellipsize_descriptions() {
        let theme = TuiTheme::default_for(super::super::theme::Appearance::Dark);
        let lines = help_panel(
            GROUPS[0].0,
            GROUPS[0].1,
            3,
            108,
            resolve_color(GROUPS[0].2, theme),
            theme,
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
    fn help_key_hints_do_not_drift_from_registered_commands() {
        let group = |label| {
            GROUPS
                .iter()
                .find(|(group, _, _)| *group == label)
                .map(|(_, entries, _)| *entries)
                .expect("help group should exist")
        };
        let entries_for = |category| match category {
            "Snippet" => group("SNIPPETS — WHEN LIST OR PREVIEW HAS FOCUS"),
            "Copy" => group("COPY"),
            "Folder" | "Tag" => group("SIDEBAR — WHEN THE LEFT PANE HAS FOCUS"),
            "View" | "Library" | "App" => group("VIEW & GLOBAL"),
            "Trash" => group("TRASH — WHEN OPEN"),
            "Git" => group("GIT CONSOLE — WHEN OPEN"),
            "Gist" => group("GIST PANEL — WHEN OPEN"),
            _ => panic!("missing help mapping for command category {category}"),
        };
        let global_entries = group("VIEW & GLOBAL");
        for command in crate::tui::command::registry() {
            let Some(hint) = command.key_hint else {
                continue;
            };
            // Console-scoped hints (Ctrl-g / Ctrl-s) keep the opening chord in the
            // global table and their key in the console's own group, regardless of
            // the command's category.
            if hint == "Ctrl-g" || hint == "Ctrl-s" {
                assert!(global_entries.iter().any(|(label, _)| *label == hint));
            } else if let Some(key) = hint.strip_prefix("Ctrl-g ") {
                assert!(global_entries.iter().any(|(label, _)| *label == "Ctrl-g"));
                assert!(
                    entries_for("Git").iter().any(|(label, _)| *label == key),
                    "{} ({hint}) is missing from Git help",
                    command.slug
                );
            } else if let Some(key) = hint.strip_prefix("Ctrl-s ") {
                assert!(global_entries.iter().any(|(label, _)| *label == "Ctrl-s"));
                assert!(
                    entries_for("Gist").iter().any(|(label, _)| *label == key),
                    "{} ({hint}) is missing from Gist help",
                    command.slug
                );
            } else {
                assert!(
                    entries_for(command.category)
                        .iter()
                        .any(|(label, _)| *label == hint),
                    "{} ({hint}) is missing from its help group",
                    command.slug
                );
            }
        }
    }
}
