use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use time::OffsetDateTime;

use crate::git::{Branch, RepoState, Unavailable};

use super::app::App;
use super::app::types::GitState;
use super::icons::IconMode;
use super::selection::text_width;
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
    let popup_width = 64.min(area.width);
    // Two border cells plus four cells of padding on each side.
    let content_width = popup_width.saturating_sub(10) as usize;
    let text = match app.git.unavailable.as_ref() {
        Some(Unavailable::NotARepository) => not_repository_text(app, content_width),
        Some(Unavailable::ProbeFailed { message }) => {
            probe_failed_text(app, message, content_width)
        }
        Some(Unavailable::BinaryMissing) | None => repository_text(app, content_width),
    };
    let content_height = u16::try_from(text.lines.len()).unwrap_or(u16::MAX);
    let popup = widgets::centered_rect(popup_width, content_height.saturating_add(2), area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(Line::from(" Git Console ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .padding(Padding::new(4, 4, 0, 0));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    frame.render_widget(Paragraph::new(text).alignment(Alignment::Left), inner);
}

fn probe_failed_text(app: &App, message: &str, width: usize) -> Text<'static> {
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

fn not_repository_text(app: &App, width: usize) -> Text<'static> {
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

fn repository_text(app: &App, width: usize) -> Text<'static> {
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
            if app.git.fetch_in_flight {
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

fn title_line(name: &str, theme: TuiTheme) -> Line<'static> {
    Line::from(Span::styled(
        name.to_owned(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ))
    .centered()
}

fn section(label: &str, width: usize, theme: TuiTheme) -> Line<'static> {
    let prefix = "── ";
    let suffix_width = width.saturating_sub(
        super::selection::text_width(prefix) as usize
            + super::selection::text_width(label) as usize
            + 1,
    );
    Line::from(vec![
        Span::styled(prefix, Style::default().fg(theme.rule)),
        Span::styled(
            label.to_owned(),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", "─".repeat(suffix_width)),
            Style::default().fg(theme.rule),
        ),
    ])
}

fn sync_verdict(status: &crate::git::Status, theme: TuiTheme) -> (String, Color) {
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

fn relationship_line(status: &crate::git::Status, width: usize, theme: TuiTheme) -> Line<'static> {
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

fn relationship_time_line(
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

fn key_value(key: &str, value: String, width: usize, theme: TuiTheme) -> Line<'static> {
    let key = format!("  {key:<12}");
    let value = widgets::truncate_end(&value, width.saturating_sub(key.len()));
    Line::from(vec![
        Span::styled(key, Style::default().fg(theme.muted)),
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
    .centered()
}

fn repository_footer(width: usize, theme: TuiTheme) -> Vec<Line<'static>> {
    let compact = width < 50;
    let entry_gap = if compact { 2 } else { 4 };
    let key_gap = if compact { 1 } else { 2 };
    let groups: &[&[(&str, &str)]] = if width < 40 {
        &[
            &[("b", "backup"), ("c", "commit")],
            &[("p", "push"), ("f", "fetch")],
            &[("i", "interval"), ("u", "auto push")],
            &[("o", "on quit"), ("C", "message")],
            &[("a", "pause"), ("r", "refresh")],
            &[("Esc", "close")],
        ]
    } else {
        &[
            &[
                ("b", "backup"),
                ("c", "commit"),
                ("p", "push"),
                ("f", "fetch"),
            ],
            &[("i", "interval"), ("u", "auto push"), ("o", "on quit")],
            &[
                ("C", "message"),
                ("a", "pause"),
                ("r", "refresh"),
                ("Esc", "close"),
            ],
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
            last_fetch_error: None,
            operation_queued: false,
            auto_attempted_at: None,
            push_attempted_at: None,
            push_in_flight: false,
            fetch_in_flight: false,
            fetched_at: None,
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
            upstream_commit: None,
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

    #[test]
    fn console_verdict_prioritizes_remote_and_worktree_risk() {
        let theme = TuiTheme::for_appearance(super::super::theme::Appearance::Dark);
        let mut current = status();
        assert_eq!(sync_verdict(&current, theme).0, "backed up and pushed");
        current.ahead = 2;
        assert!(sync_verdict(&current, theme).0.contains("not pushed"));
        current.unstaged = 1;
        assert!(sync_verdict(&current, theme).0.contains("not committed"));
        current.behind = 3;
        assert!(sync_verdict(&current, theme).0.contains("remote is 3"));
        current.conflicted.push("snippet.toml".to_owned());
        assert!(sync_verdict(&current, theme).0.contains("conflicts"));
    }

    #[test]
    fn relationship_line_distinguishes_sync_lead_lag_and_divergence() {
        let theme = TuiTheme::for_appearance(super::super::theme::Appearance::Dark);
        let text = |status: &Status| {
            relationship_line(status, 80, theme)
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        };
        let mut current = status();
        assert!(text(&current).contains("───"));
        current.ahead = 1;
        assert!(text(&current).contains("──▶"));
        current.ahead = 0;
        current.behind = 1;
        assert!(text(&current).contains("◀──"));
        current.ahead = 1;
        assert!(text(&current).contains("◀─▶"));
    }

    #[test]
    fn unicode_sections_and_narrow_footers_respect_display_width() {
        let theme = TuiTheme::for_appearance(super::super::theme::Appearance::Dark);
        for width in [30, 40, 48, 54] {
            assert_eq!(section("AUTOMATION", width, theme).width(), width);
        }
        for width in [30, 40] {
            let footer = repository_footer(width, theme);
            assert!(footer.iter().all(|line| line.width() <= width));
            let rendered = footer
                .iter()
                .flat_map(|line| line.spans.iter())
                .map(|span| span.content.as_ref())
                .collect::<String>();
            for label in [
                "backup",
                "commit",
                "push",
                "fetch",
                "interval",
                "auto push",
                "on quit",
                "message",
                "pause",
                "refresh",
                "close",
            ] {
                assert!(rendered.contains(label), "missing footer action {label}");
            }
        }
    }

    #[test]
    fn narrow_console_values_end_with_an_ellipsis() {
        let theme = TuiTheme::for_appearance(super::super::theme::Appearance::Dark);
        let value = key_value(
            "changes",
            "0 staged · 0 modified · 1 new".to_owned(),
            30,
            theme,
        );
        assert!(value.width() <= 30);
        assert!(value.spans.iter().any(|span| span.content.ends_with('…')));

        let mut current = status();
        let commit = crate::git::Commit {
            short_id: "abc1234".to_owned(),
            timestamp: 0,
            subject: "initial".to_owned(),
        };
        current.last_commit = Some(commit.clone());
        current.upstream_commit = Some(commit);
        let relationship = relationship_line(&current, 30, theme);
        assert!(relationship.width() <= 30);
        assert!(
            relationship
                .spans
                .iter()
                .any(|span| span.content.ends_with('…'))
        );
        let relationship = relationship_time_line(&current, 30, theme);
        assert!(relationship.width() <= 30);
        assert!(
            relationship
                .spans
                .iter()
                .any(|span| span.content.ends_with('…'))
        );
    }
}
