use std::collections::HashSet;

use serde::Serialize;

use super::color::{contrast, relative_luminance};
use super::{Appearance, Theme, ThemeColor};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub level: Level,
    pub detail: String,
}

fn skipped(id: &'static str) -> Check {
    Check {
        id,
        level: Level::Ok,
        detail: "skipped: terminal-defined color".to_owned(),
    }
}

pub fn check(theme: &Theme) -> Vec<Check> {
    let ui = &theme.ui;
    let distinct = [
        ui.accent,
        ui.accent_alt,
        ui.muted,
        ui.selection_bg,
        ui.retained_bg,
        ui.pill_primary,
        ui.pill_secondary,
        ui.tag,
        ui.warning,
        ui.success,
        ui.error,
    ];
    let unique = distinct.iter().collect::<HashSet<_>>().len();
    let mut checks = vec![Check {
        id: "roles-distinct",
        level: if unique == distinct.len() {
            Level::Ok
        } else {
            Level::Warn
        },
        detail: format!("{unique}/{} roles distinct", distinct.len()),
    }];

    checks.push(contrast_check(
        "selection-contrast",
        ui.selection_fg,
        ui.selection_bg,
        4.5,
        Level::Fail,
    ));
    checks.push(contrast_check(
        "foreground-contrast",
        ui.foreground,
        ui.background,
        4.5,
        Level::Fail,
    ));

    let role_pairs = [
        ("muted", ui.muted, ui.background),
        ("bar_fg", ui.bar_fg, ui.bar_bg),
        ("tag", ui.tag, ui.background),
        ("warning", ui.warning, ui.background),
        ("error", ui.error, ui.background),
        ("success", ui.success, ui.background),
    ];
    let mut known = Vec::new();
    for (name, foreground, background) in role_pairs {
        if let Some(value) = contrast(foreground, background) {
            known.push((name, value));
        }
    }
    checks.push(if known.is_empty() {
        skipped("role-legibility")
    } else {
        let failed = known
            .iter()
            .filter(|(_, value)| *value < 3.0)
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        Check {
            id: "role-legibility",
            level: if failed.is_empty() {
                Level::Ok
            } else {
                Level::Warn
            },
            detail: if failed.is_empty() {
                "all known role contrasts >= 3.0".to_owned()
            } else {
                format!("below 3.0: {}", failed.join(", "))
            },
        }
    });

    checks.push(match relative_luminance(ui.background) {
        None => skipped("appearance-luminance"),
        Some(value) => {
            let matches = (value < 0.5) == (theme.appearance == Appearance::Dark);
            Check {
                id: "appearance-luminance",
                level: if matches { Level::Ok } else { Level::Warn },
                detail: format!("{value:.2}"),
            }
        }
    });

    let paired = matches!(
        (ui.background, ui.foreground),
        (ThemeColor::Terminal, ThemeColor::Terminal)
    ) || !matches!(ui.background, ThemeColor::Terminal)
        && !matches!(ui.foreground, ThemeColor::Terminal);
    checks.push(Check {
        id: "surface-pairing",
        level: if paired { Level::Ok } else { Level::Fail },
        detail: if paired {
            "background and foreground are paired"
        } else {
            "background and foreground must both be set or both be \"terminal\""
        }
        .to_owned(),
    });
    checks
}

fn contrast_check(
    id: &'static str,
    foreground: ThemeColor,
    background: ThemeColor,
    floor: f64,
    failure: Level,
) -> Check {
    let Some(value) = contrast(foreground, background) else {
        return skipped(id);
    };
    Check {
        id,
        level: if value >= floor { Level::Ok } else { failure },
        detail: format!("{value:.2}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_selection_colors_fail() {
        let mut theme = crate::theme::load("dark-default").unwrap();
        theme.ui.selection_fg = theme.ui.selection_bg;
        assert!(
            check(&theme)
                .iter()
                .any(|item| { item.id == "selection-contrast" && item.level == Level::Fail })
        );
    }

    #[test]
    fn unpaired_surface_fails() {
        let mut theme = crate::theme::load("dark-default").unwrap();
        theme.ui.foreground = ThemeColor::Rgb(255, 255, 255);
        assert!(
            check(&theme)
                .iter()
                .any(|item| item.id == "surface-pairing" && item.level == Level::Fail)
        );
    }

    #[test]
    fn indexed_colors_skip_contrast_without_failing() {
        let mut theme = crate::theme::load("dark-default").unwrap();
        theme.ui.selection_bg = ThemeColor::Indexed(1);
        theme.ui.selection_fg = ThemeColor::Indexed(2);
        let item = check(&theme)
            .into_iter()
            .find(|item| item.id == "selection-contrast")
            .unwrap();
        assert_eq!(item.level, Level::Ok);
        assert_eq!(item.detail, "skipped: terminal-defined color");
    }
}
