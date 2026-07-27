use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap};
use time::OffsetDateTime;

use crate::git::{Branch, RepoState, Unavailable};

use super::app::App;
use super::app::types::GitState;
use super::icons::IconMode;
use super::theme::TuiTheme;
use super::widgets;

pub fn badge(state: &GitState, icons: IconMode, theme: TuiTheme) -> Option<(String, Color)> {
    let status = state.status.as_ref()?;
    if !status.conflicted.is_empty() {
        return Some((with_auto_suffix(state, "git !conflict"), theme.error));
    }
    if status.state != RepoState::Clean {
        return Some((
            with_auto_suffix(state, format!("git {}", status.state.label())),
            theme.warning,
        ));
    }
    match &status.branch {
        Branch::Detached { short_id } => {
            return Some((
                with_auto_suffix(state, format!("git detached@{short_id}")),
                theme.warning,
            ));
        }
        Branch::Unborn => {
            return Some((with_auto_suffix(state, "git no commits"), theme.muted));
        }
        Branch::Named { .. } => {}
    }

    let branch = status.branch.name().unwrap_or("git");
    let dirty = status.dirty_count();
    let (prefix, changed, up, down, clean) = match icons {
        IconMode::Nerd => ("⎇ ", " ✚", " ↑", " ↓", " ✓"),
        IconMode::Ascii => ("git:", " +", " ^", " v", " ok"),
    };
    let mut text = format!("{prefix}{branch}");
    if dirty > 0 {
        text.push_str(&format!("{changed}{dirty}"));
    }
    if status.ahead > 0 {
        text.push_str(&format!("{up}{}", status.ahead));
    }
    if status.behind > 0 {
        text.push_str(&format!("{down}{}", status.behind));
    }
    if state.push_in_flight {
        text.push_str(match icons {
            IconMode::Nerd => " ⇡",
            IconMode::Ascii => " >>",
        });
    }
    let backed_up = dirty == 0 && status.ahead == 0 && !state.push_in_flight;
    if backed_up {
        text.push_str(clean);
    }
    Some((
        with_auto_suffix(state, text),
        if backed_up {
            theme.success
        } else {
            theme.warning
        },
    ))
}

fn with_auto_suffix(state: &GitState, text: impl Into<String>) -> String {
    let mut text = text.into();
    if state.auto_commit_interval > 0 && state.auto_backup_paused {
        text.push_str(" [auto paused]");
    }
    text
}

pub fn draw_git(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let text = match app.git.unavailable.as_ref() {
        Some(Unavailable::NotARepository) => not_repository_text(app),
        Some(Unavailable::ProbeFailed { message }) => probe_failed_text(app, message),
        Some(Unavailable::BinaryMissing) | None => repository_text(app),
    };
    let content_height = u16::try_from(text.lines.len()).unwrap_or(u16::MAX);
    let popup = widgets::centered_rect(64, content_height.saturating_add(4), area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(Line::from(" Git ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .padding(Padding::new(4, 4, 1, 1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left),
        inner,
    );
}

fn probe_failed_text(app: &App, message: &str) -> Text<'static> {
    Text::from(vec![
        title_line(&app.library.manifest().name, app.theme),
        Line::raw(""),
        section("REPOSITORY", app.theme),
        Line::raw(""),
        Line::styled(
            "  Git could not inspect this library.",
            Style::default().fg(app.theme.error),
        ),
        Line::raw(""),
        Line::styled(
            format!("  {}", widgets::truncate_end(message, 50)),
            Style::default().fg(app.theme.muted),
        ),
        Line::raw(""),
        basic_footer(app.theme),
    ])
}

fn not_repository_text(app: &App) -> Text<'static> {
    let path = widgets::truncate_end(&app.library.root().display().to_string(), 52);
    Text::from(vec![
        title_line(&app.library.manifest().name, app.theme),
        Line::raw(""),
        section("REPOSITORY", app.theme),
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

fn repository_text(app: &App) -> Text<'static> {
    let Some(status) = app.git.status.as_ref() else {
        return Text::from(vec![
            title_line(&app.library.manifest().name, app.theme),
            Line::raw(""),
            section("REPOSITORY", app.theme),
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
    let mut lines = vec![
        title_line(&app.library.manifest().name, app.theme),
        Line::raw(""),
        section("REPOSITORY", app.theme),
        key_value("branch", branch_text(&status.branch), app.theme),
    ];
    if let Some(upstream) = &status.upstream {
        lines.push(key_value("upstream", upstream.clone(), app.theme));
    } else {
        lines.push(key_value(
            "upstream",
            "—  (not pushed anywhere)".to_owned(),
            app.theme,
        ));
    }
    if let Some(head) = status.head_oid.as_deref() {
        lines.push(key_value("head", head.chars().take(7).collect(), app.theme));
    }
    lines.extend([Line::raw(""), section("BACKUP", app.theme)]);
    if let Some(commit) = &status.last_commit {
        let when =
            crate::git::relative_time(commit.timestamp, OffsetDateTime::now_utc().unix_timestamp());
        lines.push(key_value("last commit", when, app.theme));
        lines.push(Line::styled(
            format!(
                "                {}",
                widgets::truncate_end(&commit.subject, 38)
            ),
            Style::default().fg(app.theme.muted),
        ));
    } else {
        lines.push(key_value("last commit", "none".to_owned(), app.theme));
    }
    lines.push(key_value(
        "unpushed",
        format!("{} commits", status.ahead),
        app.theme,
    ));
    lines.push(key_value(
        "uncommitted",
        format!(
            "{} staged · {} modified · {} new",
            status.staged, status.unstaged, status.untracked
        ),
        app.theme,
    ));
    lines.extend([Line::raw(""), section("AUTOMATIC", app.theme)]);
    let automatic_mode = if app.git.auto_commit_interval == 0 {
        "off".to_owned()
    } else if app.git.auto_push {
        format!("commit + push (every {} min)", app.git.auto_commit_interval)
    } else {
        format!("commit only (every {} min)", app.git.auto_commit_interval)
    };
    lines.push(key_value("mode", automatic_mode, app.theme));
    lines.push(key_value(
        "state",
        if app.git.auto_commit_interval == 0 {
            "off".to_owned()
        } else if app.git.auto_backup_paused {
            "paused".to_owned()
        } else {
            "active".to_owned()
        },
        app.theme,
    ));
    lines.push(key_value(
        "push",
        if app.git.push_in_flight
            && app
                .git
                .push_attempted_at
                .is_some_and(|attempt| attempt.elapsed() > std::time::Duration::from_secs(180))
        {
            "background push stalled".to_owned()
        } else if app.git.push_in_flight {
            "in flight".to_owned()
        } else {
            "idle".to_owned()
        },
        app.theme,
    ));
    lines.push(key_value(
        "on quit",
        if app.git.backup_on_quit {
            "backup enabled".to_owned()
        } else {
            "off".to_owned()
        },
        app.theme,
    ));
    let last_errors = match (
        app.git.last_commit_error.as_deref(),
        app.git.last_push_error.as_deref(),
    ) {
        (Some(commit), Some(push)) => format!(
            "C: {}; P: {}",
            widgets::truncate_end(commit, 15),
            widgets::truncate_end(push, 15)
        ),
        (Some(commit), None) => format!("commit: {}", widgets::truncate_end(commit, 30)),
        (None, Some(push)) => format!("push: {}", widgets::truncate_end(push, 32)),
        (None, None) => "—".to_owned(),
    };
    lines.push(key_value("last errors", last_errors, app.theme));

    if !status.conflicted.is_empty()
        || !matches!(status.branch, Branch::Named { .. } | Branch::Unborn)
        || status.state != RepoState::Clean
    {
        lines.extend([Line::raw(""), section("ATTENTION", app.theme)]);
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
                widgets::truncate_end(error, 52),
                Style::default().fg(app.theme.error),
            ),
        ]);
    }
    lines.extend(repository_footer(app.theme));
    Text::from(lines)
}

fn title_line(name: &str, theme: TuiTheme) -> Line<'static> {
    Line::from(Span::styled(
        name.to_owned(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ))
    .centered()
}

fn section(label: &str, theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled("── ", Style::default().fg(theme.rule)),
        Span::styled(
            label.to_owned(),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " ─────────────────────────────",
            Style::default().fg(theme.rule),
        ),
    ])
}

fn key_value(key: &str, value: String, theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key:<12}"), Style::default().fg(theme.muted)),
        Span::styled(value, Style::default().fg(theme.bar_fg)),
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
}

fn repository_footer(theme: TuiTheme) -> Vec<Line<'static>> {
    vec![
        action_line(&[("b", "backup"), ("c", "commit"), ("p", "push")], theme),
        action_line(
            &[
                ("C", "message"),
                ("a", "pause"),
                ("r", "refresh"),
                ("Esc", "close"),
            ],
            theme,
        ),
    ]
}

fn action_line(entries: &[(&str, &str)], theme: TuiTheme) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, label)) in entries.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("    ", Style::default().fg(theme.muted)));
        }
        spans.push(Span::styled(
            (*key).to_owned(),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("  {label}"),
            Style::default().fg(theme.muted),
        ));
    }
    Line::from(spans).centered()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::git::{Branch, RepoState, Status};

    use super::*;

    fn state(status: Status) -> GitState {
        GitState {
            repo: None,
            unavailable: None,
            status: Some(status),
            error: None,
            open: false,
            auto_commit_interval: 0,
            auto_push: false,
            backup_on_quit: false,
            auto_backup_paused: false,
            last_commit_error: None,
            last_push_error: None,
            operation_queued: false,
            auto_attempted_at: None,
            push_attempted_at: None,
            push_in_flight: false,
            sender: None,
            checked_at: Instant::now(),
            interval: Duration::from_secs(5),
        }
    }

    fn status() -> Status {
        Status {
            branch: Branch::Named {
                name: "main".to_owned(),
            },
            upstream: Some("origin/main".to_owned()),
            ahead: 0,
            behind: 0,
            staged: 0,
            unstaged: 0,
            untracked: 0,
            conflicted: Vec::new(),
            state: RepoState::Clean,
            head_oid: Some("abc".to_owned()),
            last_commit: None,
        }
    }

    #[test]
    fn badge_supports_ascii_and_icon_columns() {
        let theme = TuiTheme::for_appearance(super::super::theme::Appearance::Dark);
        let clean = state(status());
        assert_eq!(
            badge(&clean, IconMode::Ascii, theme).unwrap().0,
            "git:main ok"
        );
        assert_eq!(badge(&clean, IconMode::Nerd, theme).unwrap().0, "⎇ main ✓");

        let mut changed = status();
        changed.unstaged = 2;
        changed.ahead = 1;
        changed.behind = 3;
        assert_eq!(
            badge(&state(changed), IconMode::Ascii, theme).unwrap().0,
            "git:main +2 ^1 v3"
        );

        let mut pushing = state(status());
        pushing.status.as_mut().unwrap().ahead = 2;
        pushing.push_in_flight = true;
        assert_eq!(
            badge(&pushing, IconMode::Ascii, theme).unwrap().0,
            "git:main ^2 >>"
        );
        assert_eq!(
            badge(&pushing, IconMode::Nerd, theme).unwrap().0,
            "⎇ main ↑2 ⇡"
        );
    }

    #[test]
    fn badge_prioritizes_conflicts_and_special_states() {
        let theme = TuiTheme::for_appearance(super::super::theme::Appearance::Dark);
        let mut conflicted = status();
        conflicted.conflicted = vec!["snippet.toml".to_owned()];
        conflicted.state = RepoState::Merging;
        assert_eq!(
            badge(&state(conflicted), IconMode::Ascii, theme).unwrap().0,
            "git !conflict"
        );

        let mut detached = status();
        detached.branch = Branch::Detached {
            short_id: "abc1234".to_owned(),
        };
        assert_eq!(
            badge(&state(detached), IconMode::Ascii, theme).unwrap().0,
            "git detached@abc1234"
        );
    }
}
