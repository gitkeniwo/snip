use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use time::OffsetDateTime;

use crate::git::{Branch, RepoState};

use super::super::app::App;
use super::super::selection::text_width;
use super::super::theme::TuiTheme;
use super::super::widgets;

pub(super) use super::super::panel_text::{key_value, section, title_line};

pub(super) fn probe_failed_text(app: &App, message: &str, width: usize) -> Text<'static> {
    Text::from(vec![
        title_line(&app.library.manifest().name, app.theme),
        Line::raw(""),
        section("REPOSITORY", width, app.theme),
        Line::raw(""),
        Line::styled(
            "  Git could not inspect this library.",
            Style::default().fg(app.theme.error),
        ),
        Line::raw(""),
        Line::styled(
            format!(
                "  {}",
                widgets::truncate_end(message, width.saturating_sub(2))
            ),
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        basic_footer(app.theme),
    ])
}

pub(super) fn not_repository_text(app: &App, width: usize) -> Text<'static> {
    let path = widgets::truncate_end(
        &app.library.root().display().to_string(),
        width.saturating_sub(2),
    );
    Text::from(vec![
        title_line(&app.library.manifest().name, app.theme),
        Line::raw(""),
        section("REPOSITORY", width, app.theme),
        Line::raw(""),
        Line::styled(
            "  This library is not a git repository.",
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        Line::styled(format!("  {path}"), Style::default().fg(app.theme.bar_fg)),
        Line::raw(""),
        Line::styled(
            "  snip already wrote a .gitignore here.",
            Style::default().fg(app.theme.muted),
        ),
        Line::styled(
            "  To enable backups, run:",
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        Line::styled(
            "      git init",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            "  Or initialize it here:",
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        init_footer(app.theme),
    ])
}

pub(super) fn repository_text(app: &App, width: usize) -> Text<'static> {
    let Some(status) = app.git.status.as_ref() else {
        return Text::from(vec![
            title_line(&app.library.manifest().name, app.theme),
            Line::raw(""),
            section("REPOSITORY", width, app.theme),
            Line::raw(""),
            Line::styled(
                app.git
                    .error
                    .as_deref()
                    .unwrap_or("Git status is not available yet.")
                    .to_owned(),
                Style::default().fg(app.theme.error),
            ),
            Line::raw(""),
            basic_footer(app.theme),
        ]);
    };
    let (verdict, verdict_color) = sync_verdict(status, app.theme);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("● ", Style::default().fg(verdict_color)),
            Span::styled(
                verdict,
                Style::default()
                    .fg(verdict_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        relationship_line(status, width, app.theme),
        relationship_time_line(status, width, app.theme),
        Line::raw(""),
        section("WORKTREE", width, app.theme),
        key_value("branch", branch_text(&status.branch), width, app.theme),
        key_value(
            "remote",
            status.upstream.clone().unwrap_or_else(|| "none".to_owned()),
            width,
            app.theme,
        ),
        key_value(
            "changes",
            format!(
                "{} staged · {} modified · {} new",
                status.staged, status.unstaged, status.untracked
            ),
            width,
            app.theme,
        ),
        key_value(
            "last fetch",
            app.git.fetched_at.map_or_else(
                || "not fetched this session".to_owned(),
                |instant| elapsed_label(instant.elapsed()),
            ),
            width,
            app.theme,
        ),
        Line::raw(""),
        section("AUTOMATION", width, app.theme),
        checkbox_line(
            app.git.auto_commit_interval > 0,
            "i",
            if app.git.auto_commit_interval > 0 {
                format!("commit every {} min", app.git.auto_commit_interval)
            } else {
                "automatic commits off".to_owned()
            },
            app.theme,
        ),
        checkbox_line(
            app.git.auto_push,
            "u",
            "push after commit".to_owned(),
            app.theme,
        ),
        checkbox_line(
            app.git.auto_pull,
            "U",
            "pull on start".to_owned(),
            app.theme,
        ),
        checkbox_line(
            app.git.backup_on_quit,
            "o",
            "backup on quit".to_owned(),
            app.theme,
        ),
        key_value(
            "session",
            if app.git.auto_backup_paused {
                "paused (a to resume)".to_owned()
            } else {
                "active (a to pause)".to_owned()
            },
            width,
            app.theme,
        ),
        key_value(
            "network",
            if app.git.pull_in_flight {
                "pulling…".to_owned()
            } else if app.git.fetch_in_flight {
                "fetching remote status…".to_owned()
            } else if app.git.push_in_flight
                && app
                    .git
                    .push_attempted_at
                    .is_some_and(|attempt| attempt.elapsed() > std::time::Duration::from_secs(180))
            {
                "background push stalled".to_owned()
            } else if app.git.push_in_flight {
                "pushing…".to_owned()
            } else {
                "idle".to_owned()
            },
            width,
            app.theme,
        ),
    ];
    for (kind, error) in [
        ("commit error", app.git.last_commit_error.as_deref()),
        ("push error", app.git.last_push_error.as_deref()),
        ("fetch error", app.git.last_fetch_error.as_deref()),
    ] {
        if let Some(error) = error {
            lines.push(key_value(kind, error.to_owned(), width, app.theme));
        }
    }

    if !status.conflicted.is_empty()
        || !matches!(status.branch, Branch::Named { .. } | Branch::Unborn)
        || status.state != RepoState::Clean
    {
        lines.extend([Line::raw(""), section("ATTENTION", width, app.theme)]);
        if status.state != RepoState::Clean {
            lines.push(Line::styled(
                format!("  repository is in {} state", status.state.label()),
                Style::default().fg(app.theme.warning),
            ));
        }
        if matches!(status.branch, Branch::Detached { .. }) {
            lines.push(Line::styled(
                "  HEAD is detached",
                Style::default().fg(app.theme.warning),
            ));
        }
        if !status.conflicted.is_empty() {
            lines.push(Line::styled(
                format!("  conflicts in {} files", status.conflicted.len()),
                Style::default().fg(app.theme.error),
            ));
            for path in status.conflicted.iter().take(5) {
                lines.push(Line::styled(
                    format!("    {}", widgets::truncate_end(path, 46)),
                    Style::default().fg(app.theme.muted),
                ));
            }
            if status.conflicted.len() > 5 {
                lines.push(Line::styled(
                    format!("    … and {} more", status.conflicted.len() - 5),
                    Style::default().fg(app.theme.muted),
                ));
            }
        }
    }
    if let Some(error) = &app.git.error {
        lines.extend([
            Line::raw(""),
            Line::styled(
                widgets::truncate_end(error, width),
                Style::default().fg(app.theme.error),
            ),
        ]);
    }
    lines.extend(repository_footer(width, app.theme));
    Text::from(lines)
}

pub(super) fn sync_verdict(status: &crate::git::Status, theme: TuiTheme) -> (String, Color) {
    if !status.conflicted.is_empty() {
        return (
            format!(
                "{} conflicts need terminal resolution",
                status.conflicted.len()
            ),
            theme.error,
        );
    }
    if status.state != RepoState::Clean {
        return (
            format!("repository is in {} state", status.state.label()),
            theme.error,
        );
    }
    if matches!(status.branch, Branch::Detached { .. }) {
        return ("HEAD is detached".to_owned(), theme.warning);
    }
    if status.behind > 0 {
        return (
            format!("remote is {} commit(s) ahead", status.behind),
            theme.warning,
        );
    }
    if status.dirty_count() > 0 {
        return (
            format!("{} change(s) not committed", status.dirty_count()),
            theme.warning,
        );
    }
    if status.ahead > 0 {
        return (
            format!("{} commit(s) not pushed", status.ahead),
            theme.warning,
        );
    }
    if status.upstream.is_none() {
        return (
            "committed locally; no remote configured".to_owned(),
            theme.muted,
        );
    }
    ("backed up and pushed".to_owned(), theme.success)
}

pub(super) fn relationship_line(
    status: &crate::git::Status,
    width: usize,
    theme: TuiTheme,
) -> Line<'static> {
    let local = status
        .last_commit
        .as_ref()
        .map_or("none", |commit| commit.short_id.as_str());
    let remote = status
        .upstream_commit
        .as_ref()
        .map_or("none", |commit| commit.short_id.as_str());
    let arrow = match (status.ahead > 0, status.behind > 0) {
        (true, true) => " ◀─▶ ",
        (true, false) => " ──▶ ",
        (false, true) => " ◀── ",
        (false, false) => " ─── ",
    };
    let local = format!("local {local}");
    let remote = format!(
        "{} {remote}",
        status.upstream.as_deref().unwrap_or("remote")
    );
    let remote_width =
        width.saturating_sub(text_width(&local) as usize + text_width(arrow) as usize);
    let remote = if remote_width == 0 {
        String::new()
    } else {
        widgets::truncate_end(&remote, remote_width)
    };
    Line::from(vec![
        Span::styled(local, Style::default().fg(theme.bar_fg)),
        Span::styled(arrow, Style::default().fg(theme.accent)),
        Span::styled(remote, Style::default().fg(theme.bar_fg)),
    ])
    .centered()
}

pub(super) fn relationship_time_line(
    status: &crate::git::Status,
    width: usize,
    theme: TuiTheme,
) -> Line<'static> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let local = status.last_commit.as_ref().map_or_else(
        || "local none".to_owned(),
        |commit| format!("local {}", crate::git::relative_time(commit.timestamp, now)),
    );
    let remote = status.upstream_commit.as_ref().map_or_else(
        || "remote unknown".to_owned(),
        |commit| {
            format!(
                "remote {}",
                crate::git::relative_time(commit.timestamp, now)
            )
        },
    );
    Line::styled(
        widgets::truncate_end(&format!("{local} · {remote}"), width),
        Style::default().fg(theme.muted),
    )
    .centered()
}

fn elapsed_label(elapsed: std::time::Duration) -> String {
    let seconds = i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX);
    crate::git::relative_time(0, seconds)
}

fn checkbox_line(checked: bool, key: &str, label: String, theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            if checked { "  [x] " } else { "  [ ] " },
            Style::default().fg(if checked { theme.success } else { theme.muted }),
        ),
        Span::styled(
            key.to_owned(),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {label}"), Style::default().fg(theme.bar_fg)),
    ])
}

fn branch_text(branch: &Branch) -> String {
    match branch {
        Branch::Named { name } => name.clone(),
        Branch::Detached { short_id } => format!("detached@{short_id}"),
        Branch::Unborn => "no commits".to_owned(),
    }
}

fn basic_footer(theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "Esc",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  close    ", Style::default().fg(theme.muted)),
        Span::styled(
            "r",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  refresh", Style::default().fg(theme.muted)),
    ])
    .centered()
}

fn init_footer(theme: TuiTheme) -> Line<'static> {
    action_line(
        &[("i", "initialize"), ("r", "refresh"), ("Esc", "close")],
        theme,
    )
    .centered()
}

pub(super) fn repository_footer(width: usize, theme: TuiTheme) -> Vec<Line<'static>> {
    let compact = width < 50;
    let entry_gap = if compact { 2 } else { 4 };
    let key_gap = if compact { 1 } else { 2 };
    let groups: &[&[(&str, &str)]] = if width < 40 {
        &[
            &[("b", "backup"), ("c", "commit")],
            &[("p", "push"), ("l", "pull")],
            &[("f", "fetch"), ("r", "refresh")],
            &[("i", "interval"), ("u", "auto push")],
            &[("U", "auto pull"), ("o", "on quit")],
            &[("C", "message"), ("a", "pause")],
            &[("Esc", "close")],
        ]
    } else {
        &[
            &[
                ("b", "backup"),
                ("c", "commit"),
                ("p", "push"),
                ("l", "pull"),
            ],
            &[
                ("f", "fetch"),
                ("i", "interval"),
                ("u", "auto push"),
                ("U", "auto pull"),
            ],
            &[
                ("o", "on quit"),
                ("C", "message"),
                ("a", "pause"),
                ("r", "refresh"),
            ],
            &[("Esc", "close")],
        ]
    };
    let mut lines = groups
        .iter()
        .map(|entries| action_line_with_spacing(entries, theme, entry_gap, key_gap))
        .collect::<Vec<_>>();
    let max_width = lines.iter().map(Line::width).max().unwrap_or(0);
    for line in &mut lines {
        line.alignment = Some(Alignment::Center);
        let padding = max_width.saturating_sub(line.width());
        if padding > 0 {
            line.spans.push(Span::raw(" ".repeat(padding)));
        }
    }
    lines
}

fn action_line(entries: &[(&str, &str)], theme: TuiTheme) -> Line<'static> {
    action_line_with_spacing(entries, theme, 4, 2)
}

fn action_line_with_spacing(
    entries: &[(&str, &str)],
    theme: TuiTheme,
    entry_gap: usize,
    key_gap: usize,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, label)) in entries.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(
                " ".repeat(entry_gap),
                Style::default().fg(theme.muted),
            ));
        }
        spans.push(Span::styled(
            (*key).to_owned(),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{}{label}", " ".repeat(key_gap)),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans)
}
