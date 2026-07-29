use ratatui::style::Color;

use crate::git::{Branch, RepoState};

use super::super::app::types::GitState;
use super::super::icons::IconMode;
use super::super::theme::TuiTheme;

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
