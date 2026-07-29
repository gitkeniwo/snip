mod badge;
mod render;
mod text;

pub use badge::badge;
pub use render::draw_git;

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::git::{Branch, RepoState, Status};

    use super::super::app::types::GitState;
    use super::super::icons::IconMode;
    use super::super::theme::TuiTheme;
    use super::badge::badge;
    use super::text::{
        key_value, relationship_line, relationship_time_line, repository_footer, section,
        sync_verdict,
    };

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
