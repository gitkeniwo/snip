use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::keys::{Keymap, Mode};
use crate::tui::command::CommandId;

use super::super::app::App;
use super::super::panel_text::{key_value, section};
use super::super::theme::TuiTheme;
use super::super::widgets;
use super::GistBadge;

pub fn gist_panel_text(app: &App, width: usize) -> Text<'static> {
    if let Some(reason) = unavailable_reason(app.gist.unavailable.as_ref()) {
        return unavailable_text(app, reason);
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

fn unavailable_text(app: &App, reason: &str) -> Text<'static> {
    Text::from(vec![
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
        close_footer(app),
    ])
}

fn no_record_text(
    app: &App,
    snippet: Option<&crate::domain::Snippet>,
    width: usize,
) -> Text<'static> {
    let title = snippet.map_or("", |snippet| snippet.title.as_str());
    Text::from(vec![
        Line::raw(""),
        field("title", title, width, app.theme, true),
        key_value("gist", "not published".to_owned(), width, app.theme),
        Line::raw(""),
        section("ACTIONS", width, app.theme),
        Line::raw(""),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistPush)]),
            "publish as a secret gist",
            app.theme,
            true,
        ),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistPushPublic)]),
            "publish as a public gist",
            app.theme,
            true,
        ),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistAttach)]),
            "link an existing gist…",
            app.theme,
            true,
        ),
        Line::raw(""),
        close_footer(app),
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
            Some(GistBadge::Synced) => ("clean", app.theme.bar_fg),
            Some(GistBadge::Modified) | None => ("modified", app.theme.warning),
        }
    };
    Text::from(vec![
        Line::raw(""),
        field("title", &snippet.title, width, app.theme, true),
        field("url", &record.url, width, app.theme, false),
        Line::raw(""),
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
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistPush)]),
            "update the gist",
            app.theme,
            true,
        ),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistCopyUrl)]),
            "copy link",
            app.theme,
            true,
        ),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistOpenInBrowser)]),
            "open in browser",
            app.theme,
            true,
        ),
        Line::raw(""),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistVerifyRemote)]),
            "check it still exists",
            app.theme,
            true,
        ),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistDetach)]),
            "unlink (keeps the gist)…",
            app.theme,
            true,
        ),
        action_line(
            binding_label(&app.keymap, &[(Mode::Gist, CommandId::GistDelete)]),
            "delete on GitHub…",
            app.theme,
            true,
        ),
        Line::raw(""),
        close_footer(app),
    ])
}

fn pushed_value(pushed_at: Option<&str>) -> String {
    let Some(pushed_at) = pushed_at else {
        return String::new();
    };
    match OffsetDateTime::parse(pushed_at, &Rfc3339) {
        // The full timestamp is noise at this size; `snip gist status` has it.
        Ok(value) => crate::git::relative_time(
            value.unix_timestamp(),
            OffsetDateTime::now_utc().unix_timestamp(),
        ),
        Err(_) => pushed_at.to_owned(),
    }
}

/// A labelled field on the same 12-cell key column as [`key_value`], so the
/// title and url line up with the fields below them.
fn field(key: &str, value: &str, width: usize, theme: TuiTheme, bold: bool) -> Line<'static> {
    let key = format!("  {key:<12}");
    let value = widgets::truncate_end(value, width.saturating_sub(key.len()));
    let mut style = Style::default().fg(theme.bar_fg);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Line::from(vec![
        Span::styled(key, Style::default().fg(theme.muted)),
        Span::styled(value, style),
    ])
}

fn state_line(state: &str, color: Color, width: usize, theme: TuiTheme) -> Line<'static> {
    let key = format!("  {:<12}", "state");
    let value = widgets::truncate_end(state, width.saturating_sub(key.len()));
    Line::from(vec![
        Span::styled(key, Style::default().fg(theme.muted)),
        Span::styled(value, Style::default().fg(color)),
    ])
}

/// One action per line. `primary` marks the everyday verbs so they carry more
/// weight than the occasional ones sharing the panel.
fn action_line(key: String, label: &str, theme: TuiTheme, primary: bool) -> Line<'static> {
    let (key_style, label_style) = if primary {
        (
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(theme.bar_fg),
        )
    } else {
        (
            Style::default().fg(theme.muted),
            Style::default().fg(theme.muted),
        )
    };
    Line::from(vec![
        Span::raw("  "),
        Span::styled(key, key_style),
        Span::styled("  ", label_style),
        Span::styled(label.to_owned(), label_style),
    ])
}

fn close_footer(app: &App) -> Line<'static> {
    let dismiss = binding_label(&app.keymap, &[(Mode::Gist, CommandId::UiDismiss)]);
    let toggle = binding_label(&app.keymap, &[(Mode::Global, CommandId::GistTogglePanel)]);
    Line::from(vec![
        Span::styled(
            dismiss,
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" / ", Style::default().fg(app.theme.muted)),
        Span::styled(
            toggle,
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  close", Style::default().fg(app.theme.muted)),
    ])
    .centered()
}

fn binding_label(keymap: &Keymap, bindings: &[(Mode, CommandId)]) -> String {
    let mut labels = Vec::new();
    for (mode, command) in bindings {
        for chord in keymap.chords_for(&[*mode], *command) {
            let label = chord.display();
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    if labels.is_empty() {
        "—".to_owned()
    } else {
        labels.join(" / ")
    }
}
