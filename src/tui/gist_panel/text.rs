use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::super::app::App;
use super::super::panel_text::{key_value, section};
use super::super::selection::text_width;
use super::super::theme::TuiTheme;
use super::super::widgets;
use super::GistBadge;

pub fn gist_panel_text(app: &App, width: usize) -> Text<'static> {
    if let Some(reason) = unavailable_reason(app.gist.unavailable.as_ref()) {
        return unavailable_text(app, reason, width);
    }
    match app.selected_snippet() {
        Some(snippet) => match crate::gist::find(snippet) {
            Some(record) => record_text(app, snippet, record, width),
            None => no_record_text(app, Some(snippet), width),
        },
        None => no_record_text(app, None, width),
    }
}

fn unavailable_reason(unavailable: Option<&crate::gist::gh::Unavailable>) -> Option<&'static str> {
    match unavailable {
        Some(crate::gist::gh::Unavailable::BinaryMissing) => Some("gh was not found in PATH."),
        Some(crate::gist::gh::Unavailable::NotAuthenticated) => Some("gh is not authenticated."),
        Some(crate::gist::gh::Unavailable::Failed { .. }) | None => None,
    }
}

fn unavailable_text(app: &App, reason: &str, width: usize) -> Text<'static> {
    Text::from(vec![
        section("GIST", width, app.theme),
        Line::raw(""),
        Line::styled(format!("  {reason}"), Style::default().fg(app.theme.error)),
        Line::raw(""),
        Line::styled(
            "  snip publishes gists through the GitHub CLI.",
            Style::default().fg(app.theme.muted),
        ),
        Line::styled(
            "  Install it from https://cli.github.com, then run:",
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        Line::styled(
            "    gh auth login",
            Style::default().fg(app.theme.accent_alt),
        ),
        Line::styled(
            "    gh auth refresh -h github.com -s gist",
            Style::default().fg(app.theme.accent_alt),
        ),
        Line::raw(""),
        close_footer(app.theme),
    ])
}

fn no_record_text(
    app: &App,
    snippet: Option<&crate::domain::Snippet>,
    width: usize,
) -> Text<'static> {
    let title = snippet.map_or("", |snippet| snippet.title.as_str());
    Text::from(vec![
        section("SNIPPET", width, app.theme),
        Line::raw(""),
        Line::styled(format!("  {title}"), Style::default().fg(app.theme.bar_fg)),
        Line::raw(""),
        section("GIST", width, app.theme),
        Line::raw(""),
        Line::styled(
            "  This snippet has not been published.",
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        section("ACTIONS", width, app.theme),
        Line::raw(""),
        action_row(
            &[("p", "push"), ("a", "attach")],
            15,
            primary_key(app.theme),
            primary_label(app.theme),
        ),
        Line::raw(""),
        action_row(
            &[("f", "published only")],
            11,
            secondary_key(app.theme),
            secondary_label(app.theme),
        ),
        Line::raw(""),
        close_footer(app.theme),
    ])
}

fn record_text(
    app: &App,
    snippet: &crate::domain::Snippet,
    record: &crate::domain::RemoteRecord,
    width: usize,
) -> Text<'static> {
    let state = if app.gist.last_error.as_deref() == Some("gist no longer exists on GitHub") {
        ("missing", app.theme.error)
    } else {
        match app.gist_badges.get(&snippet.id) {
            Some(GistBadge::Synced) => ("clean", app.theme.muted),
            Some(GistBadge::Modified) | None => ("modified", app.theme.warning),
        }
    };
    Text::from(vec![
        section("SNIPPET", width, app.theme),
        Line::raw(""),
        Line::styled(
            format!("  {}", snippet.title),
            Style::default().fg(app.theme.bar_fg),
        ),
        Line::raw(""),
        section("GIST", width, app.theme),
        Line::raw(""),
        key_value("url", record.url.clone(), width, app.theme),
        key_value(
            "visibility",
            if record.public {
                "public".to_owned()
            } else {
                "secret".to_owned()
            },
            width,
            app.theme,
        ),
        state_line(state.0, state.1, width, app.theme),
        key_value(
            "pushed",
            pushed_value(record.pushed_at.as_deref()),
            width,
            app.theme,
        ),
        Line::raw(""),
        section("ACTIONS", width, app.theme),
        Line::raw(""),
        action_row(
            &[("p", "push"), ("y", "copy URL"), ("o", "open in browser")],
            15,
            primary_key(app.theme),
            primary_label(app.theme),
        ),
        Line::raw(""),
        action_row(
            &[
                ("a", "attach"),
                ("d", "detach"),
                ("x", "delete"),
                ("r", "verify"),
                ("f", "published only"),
            ],
            11,
            secondary_key(app.theme),
            secondary_label(app.theme),
        ),
        Line::raw(""),
        close_footer(app.theme),
    ])
}

fn pushed_value(pushed_at: Option<&str>) -> String {
    let Some(pushed_at) = pushed_at else {
        return String::new();
    };
    match OffsetDateTime::parse(pushed_at, &Rfc3339) {
        Ok(value) => {
            let relative = crate::git::relative_time(
                value.unix_timestamp(),
                OffsetDateTime::now_utc().unix_timestamp(),
            );
            format!("{pushed_at} ({relative})")
        }
        Err(_) => pushed_at.to_owned(),
    }
}

fn state_line(state: &str, color: Color, width: usize, theme: TuiTheme) -> Line<'static> {
    let key = format!("  {:<12}", "state");
    let value = widgets::truncate_end(state, width.saturating_sub(key.len()));
    Line::from(vec![
        Span::styled(key, Style::default().fg(theme.muted)),
        Span::styled(value, Style::default().fg(color)),
    ])
}

fn primary_key(theme: TuiTheme) -> Style {
    Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

fn primary_label(theme: TuiTheme) -> Style {
    Style::default().fg(theme.bar_fg)
}

fn secondary_key(theme: TuiTheme) -> Style {
    Style::default().fg(theme.muted)
}

fn secondary_label(theme: TuiTheme) -> Style {
    Style::default().fg(theme.muted)
}

fn action_row(
    entries: &[(&str, &str)],
    cell_width: usize,
    key_style: Style,
    label_style: Style,
) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    for (key, label) in entries {
        let cell = format!("{key}  {label}");
        let padding = cell_width.saturating_sub(text_width(&cell) as usize);
        spans.push(Span::styled((*key).to_owned(), key_style));
        spans.push(Span::styled("  ", label_style));
        spans.push(Span::styled((*label).to_owned(), label_style));
        spans.push(Span::styled(" ".repeat(padding), label_style));
    }
    Line::from(spans)
}

fn close_footer(theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "Esc",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" / ", Style::default().fg(theme.muted)),
        Span::styled(
            "Ctrl-s",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  close", Style::default().fg(theme.muted)),
    ])
    .centered()
}
