use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

use super::selection::text_width;
use super::theme::TuiTheme;
use super::widgets;

pub fn draw_help(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    let popup = widgets::centered_rect(108, 38, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(Line::from(" Help ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .padding(Padding::new(4, 4, 1, 1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(6),
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "snip TUI",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .centered(),
            Line::from(Span::styled(
                "keyboard & mouse reference",
                Style::default().fg(theme.muted),
            ))
            .centered(),
        ])),
        rows[0],
    );

    frame.render_widget(
        help_panel(
            "NAVIGATION",
            &[
                ("Tab", "next pane"),
                ("Shift-Tab", "previous pane"),
                ("h / ←", "back"),
                ("l / →", "drill in"),
                ("j / ↓", "next item"),
                ("k / ↑", "previous item"),
                ("g", "first item"),
                ("G", "last item"),
                ("Ctrl-d", "page down"),
                ("Ctrl-u", "page up"),
                ("[", "previous fragment"),
                ("]", "next fragment"),
            ],
            theme.accent,
            theme,
        ),
        rows[2],
    );
    frame.render_widget(
        help_panel(
            "SNIPPETS",
            &[
                ("n", "new snippet"),
                ("e", "edit content"),
                ("E", "edit note"),
                ("R", "edit README"),
                ("r", "rename snippet"),
                ("m", "move snippet"),
                ("t", "edit tags"),
                ("d", "move to trash"),
                ("p", "toggle pin"),
                ("L", "toggle lock"),
                ("y", "copy content"),
                ("Y", "copy UUID"),
            ],
            theme.accent_alt,
            theme,
        ),
        rows[4],
    );

    frame.render_widget(
        help_panel(
            "LIBRARY & GLOBAL",
            &[
                ("n", "new child folder"),
                ("r", "rename folder or tag"),
                ("d", "delete folder or tag"),
                ("/", "search"),
                ("s", "cycle sort"),
                ("T", "open trash"),
                ("F5 / Ctrl-r", "rescan"),
                ("Esc", "close or clear"),
                ("q", "quit"),
                ("?", "toggle help"),
            ],
            theme.tag,
            theme,
        ),
        rows[6],
    );
    frame.render_widget(
        help_panel(
            "PREVIEW & MOUSE",
            &[
                ("N", "toggle line numbers"),
                ("wheel", "scroll hovered pane"),
                ("click", "select item or tab"),
                ("double-click", "drill into preview"),
                ("drag", "select preview text"),
                ("mouse up", "copy selection"),
            ],
            theme.success,
            theme,
        ),
        rows[8],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Esc",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  close help", Style::default().fg(theme.muted)),
        ]))
        .alignment(Alignment::Center),
        rows[10],
    );
}

fn help_panel(
    label: &str,
    entries: &[(&str, &str)],
    key_color: ratatui::style::Color,
    theme: TuiTheme,
) -> Paragraph<'static> {
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
    lines.extend(entries.chunks(2).map(|pair| {
        let left = pair[0];
        let right = pair.get(1).copied().unwrap_or(("", ""));
        Line::from(vec![
            Span::styled(
                format!("  {}", pad_display(left.0, 15)),
                Style::default().fg(key_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(pad_display(left.1, 30), Style::default().fg(theme.muted)),
            Span::styled(
                pad_display(right.0, 15),
                Style::default().fg(key_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(right.1.to_owned(), Style::default().fg(theme.muted)),
        ])
    }));
    Paragraph::new(Text::from(lines))
}

fn pad_display(value: &str, width: usize) -> String {
    let used = text_width(value) as usize;
    format!("{value}{}", " ".repeat(width.saturating_sub(used)))
}
