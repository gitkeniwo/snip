use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::app::App;
use super::modal::Modal;
use super::selection::text_width;
use super::state::{Pane, StatusLevel};
use super::theme::TuiTheme;
use super::widgets;

type Shortcut<'a> = (&'a str, &'a str);
type ShortcutSet<'a> = &'a [Shortcut<'a>];

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
                    Span::raw(picker.filter.clone()),
                    Span::styled(
                        "  Enter select  Esc cancel",
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
    let navigation_full: ShortcutSet<'_> = &[
        ("←/→", "nav"),
        ("Tab", "pane"),
        ("/", "search"),
        ("?", "help"),
        ("q", "quit"),
    ];
    let navigation_medium: ShortcutSet<'_> = &[
        ("←/→", "nav"),
        ("Tab", "pane"),
        ("/", "search"),
        ("?", "help"),
    ];
    let navigation_compact: ShortcutSet<'_> = &[("←/→", ""), ("Tab", ""), ("/", ""), ("?", "")];
    let navigation_minimal: ShortcutSet<'_> = &[("Tab", ""), ("/", "")];

    let (actions_full, actions_medium, actions_compact): (
        ShortcutSet<'_>,
        ShortcutSet<'_>,
        ShortcutSet<'_>,
    ) = if app.trash.open {
        (
            &[("j/k", "move"), ("u", "restore"), ("x", "purge")],
            &[("u", "restore"), ("x", "purge")],
            &[("u", ""), ("x", "")],
        )
    } else {
        match app.focus {
            Pane::Sidebar => (
                &[
                    ("n", "new"),
                    ("r", "rename"),
                    ("d", "delete"),
                    ("s", "sort"),
                ],
                &[("n", "new"), ("r", "rename"), ("d", "delete")],
                &[("n", ""), ("r", ""), ("d", "")],
            ),
            Pane::List => (
                &[
                    ("n", "new"),
                    ("e", "edit"),
                    ("v", "vscode"),
                    ("r", "rename"),
                    ("m", "move"),
                    ("t", "tags"),
                    ("P", "path"),
                ],
                &[("n", "new"), ("e", "edit"), ("v", "vscode"), ("P", "path")],
                &[("n", ""), ("e", ""), ("v", ""), ("P", "")],
            ),
            Pane::Preview => (
                &[
                    ("e", "code"),
                    ("v", "vscode"),
                    ("E", "note"),
                    ("R", "readme"),
                    ("N", "lines"),
                    ("y", "copy"),
                    ("P", "path"),
                ],
                &[("e", "edit"), ("v", "vscode"), ("y", "copy"), ("P", "path")],
                &[("e", ""), ("v", ""), ("y", ""), ("P", "")],
            ),
        }
    };

    let tiers = [
        (navigation_full, actions_full),
        (navigation_medium, actions_medium),
        (navigation_compact, actions_medium),
        (navigation_compact, actions_compact),
        (navigation_minimal, &actions_compact[..1]),
    ];
    let (navigation, actions) = tiers
        .into_iter()
        .find(|(navigation, actions)| {
            shortcut_pills_width(navigation) + shortcut_pills_width(actions) + 2
                <= area.width as usize
        })
        .unwrap_or((navigation_minimal, &actions_compact[..1]));

    let left = shortcut_pills(navigation, app.theme);
    let right = shortcut_pills(actions, app.theme);
    let right_width = right.width().min(area.width as usize) as u16;
    let regions =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);
    frame.render_widget(Paragraph::new(left), regions[0]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        regions[1],
    );
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
    let primary = theme.pill_primary;
    let secondary = theme.pill_secondary;
    let mut spans = Vec::new();
    for (index, (key, action)) in commands.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" ", Style::default().bg(theme.bar_bg)));
        }
        spans.push(widgets::pill_cap(widgets::PILL_OPEN, primary, theme.bar_bg));
        spans.push(Span::styled(
            (*key).to_owned(),
            Style::default()
                .fg(theme.selection_fg)
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
                    .fg(primary)
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
