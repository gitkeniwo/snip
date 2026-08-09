use ratatui::style::{Color, Modifier, Style};

use crate::config::{TuiConfig, TuiThemeSetting};
use crate::theme::{NamedColor, Theme, ThemeColor};

pub use crate::theme::Appearance;
pub use crate::theme::resolve_appearance;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TuiTheme {
    pub appearance: Appearance,
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub accent: Color,
    pub accent_alt: Color,
    pub border: Color,
    pub muted: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub retained_bg: Color,
    pub pill_primary: Color,
    pub pill_secondary: Color,
    pub bar_bg: Color,
    pub bar_fg: Color,
    pub tag: Color,
    pub rule: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl TuiTheme {
    pub fn detect() -> Self {
        Self::resolve(TuiThemeSetting::Auto)
    }

    pub fn resolve(setting: TuiThemeSetting) -> Self {
        let config = TuiConfig {
            theme: setting,
            ..TuiConfig::default()
        };
        let (theme, _) = crate::theme::resolve(&config);
        Self::from(&theme)
    }

    pub fn default_for(appearance: Appearance) -> Self {
        let name = match appearance {
            Appearance::Light => "light-default",
            Appearance::Dark => "dark-default",
        };
        Self::from(&crate::theme::load(name).expect("built-in default theme must parse"))
    }

    pub fn with_overrides(mut self, overrides: &toml::Table) -> Self {
        let original_surface = (self.background, self.foreground);
        macro_rules! apply {
            ($field:ident) => {
                if let Some(value) = overrides
                    .get(stringify!($field))
                    .and_then(toml::Value::as_str)
                    && let Ok(value) = ThemeColor::parse(value, stringify!($field))
                {
                    self.$field = color(value);
                }
            };
        }
        apply!(accent);
        apply!(accent_alt);
        apply!(border);
        apply!(muted);
        apply!(selection_bg);
        apply!(selection_fg);
        apply!(retained_bg);
        apply!(pill_primary);
        apply!(pill_secondary);
        apply!(bar_bg);
        apply!(bar_fg);
        apply!(tag);
        apply!(rule);
        apply!(success);
        apply!(warning);
        apply!(error);
        for (role, target) in [
            ("background", &mut self.background),
            ("foreground", &mut self.foreground),
        ] {
            if let Some(value) = overrides.get(role).and_then(toml::Value::as_str)
                && let Ok(value) = ThemeColor::parse(value, role)
            {
                *target = optional_color(value);
            }
        }
        if self.background.is_some() != self.foreground.is_some() {
            (self.background, self.foreground) = original_surface;
        }
        self
    }

    pub fn surface_style(self) -> Option<Style> {
        if self.background.is_none() && self.foreground.is_none() {
            return None;
        }
        let mut style = Style::default();
        if let Some(background) = self.background {
            style = style.bg(background);
        }
        if let Some(foreground) = self.foreground {
            style = style.fg(foreground);
        }
        Some(style)
    }

    pub fn selected(self) -> Style {
        Style::default()
            .fg(self.legible_on(self.selection_bg, self.selection_fg))
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn retained_selection(self) -> Style {
        Style::default()
            .fg(self.legible_on(self.retained_bg, self.accent))
            .bg(self.retained_bg)
            .add_modifier(Modifier::BOLD)
    }

    /// Foreground that stays legible on `background`.
    ///
    /// Returns `preferred` when it already clears the 4.5 body-text floor.
    /// Otherwise picks the most legible of the theme's own surface colours,
    /// falling back to black or white. Returns `preferred` unchanged when the
    /// background is terminal-defined and its luminance is unknowable.
    pub fn legible_on(self, background: Color, preferred: Color) -> Color {
        let Some(background_luminance) = luminance(background) else {
            return preferred;
        };
        if contrast_with_luminance(preferred, background_luminance)
            .is_some_and(|value| value >= 4.5)
        {
            return preferred;
        }

        [
            self.foreground,
            self.background,
            Some(Color::Rgb(0, 0, 0)),
            Some(Color::Rgb(255, 255, 255)),
        ]
        .into_iter()
        .flatten()
        .filter_map(|candidate| {
            contrast_with_luminance(candidate, background_luminance).map(|value| (candidate, value))
        })
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(candidate, _)| candidate)
        .expect("black and white always have known luminance")
    }
}

impl From<&Theme> for TuiTheme {
    fn from(theme: &Theme) -> Self {
        let ui = &theme.ui;
        Self {
            appearance: theme.appearance,
            background: optional_color(ui.background),
            foreground: optional_color(ui.foreground),
            accent: color(ui.accent),
            accent_alt: color(ui.accent_alt),
            border: color(ui.border),
            muted: color(ui.muted),
            selection_bg: color(ui.selection_bg),
            selection_fg: color(ui.selection_fg),
            retained_bg: color(ui.retained_bg),
            pill_primary: color(ui.pill_primary),
            pill_secondary: inherited_color(ui.pill_secondary),
            bar_bg: color(ui.bar_bg),
            bar_fg: color(ui.bar_fg),
            tag: color(ui.tag),
            rule: color(ui.rule),
            success: color(ui.success),
            warning: color(ui.warning),
            error: color(ui.error),
        }
    }
}

fn optional_color(value: ThemeColor) -> Option<Color> {
    match value {
        ThemeColor::Terminal => None,
        value => Some(color(value)),
    }
}

fn inherited_color(value: ThemeColor) -> Color {
    match value {
        ThemeColor::Terminal => Color::Reset,
        value => color(value),
    }
}

fn color(value: ThemeColor) -> Color {
    match value {
        ThemeColor::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
        ThemeColor::Indexed(index) => Color::Indexed(index),
        ThemeColor::Named(named) => match named {
            NamedColor::Black => Color::Black,
            NamedColor::Red => Color::Red,
            NamedColor::Green => Color::Green,
            NamedColor::Yellow => Color::Yellow,
            NamedColor::Blue => Color::Blue,
            NamedColor::Magenta => Color::Magenta,
            NamedColor::Cyan => Color::Cyan,
            NamedColor::White => Color::White,
            NamedColor::BrightBlack => Color::DarkGray,
            NamedColor::BrightRed => Color::LightRed,
            NamedColor::BrightGreen => Color::LightGreen,
            NamedColor::BrightYellow => Color::LightYellow,
            NamedColor::BrightBlue => Color::LightBlue,
            NamedColor::BrightMagenta => Color::LightMagenta,
            NamedColor::BrightCyan => Color::LightCyan,
            NamedColor::BrightWhite => Color::Gray,
        },
        ThemeColor::Terminal => panic!("terminal color is only legal for optional surface roles"),
    }
}

fn luminance(color: Color) -> Option<f64> {
    let [red, green, blue] = match color {
        Color::Rgb(red, green, blue) => [red, green, blue],
        Color::Indexed(index @ 16..=231) => {
            const COMPONENTS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let index = index - 16;
            [
                COMPONENTS[usize::from(index / 36)],
                COMPONENTS[usize::from(index % 36 / 6)],
                COMPONENTS[usize::from(index % 6)],
            ]
        }
        Color::Indexed(index @ 232..=255) => {
            let value = 8 + 10 * (index - 232);
            [value, value, value]
        }
        _ => return None,
    };
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue))
}

#[cfg(test)]
fn contrast(left: Color, right: Color) -> Option<f64> {
    let left = luminance(left)?;
    let right = luminance(right)?;
    Some((left.max(right) + 0.05) / (left.min(right) + 0.05))
}

fn contrast_with_luminance(color: Color, background_luminance: f64) -> Option<f64> {
    let color_luminance = luminance(color)?;
    Some(
        (color_luminance.max(background_luminance) + 0.05)
            / (color_luminance.min(background_luminance) + 0.05),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_override_precedes_explicit_theme() {
        assert_eq!(
            resolve_appearance(TuiThemeSetting::Dark, Some("light")),
            Appearance::Light
        );
        assert_eq!(
            resolve_appearance(TuiThemeSetting::Light, Some("dark")),
            Appearance::Dark
        );
        assert_eq!(
            resolve_appearance(TuiThemeSetting::Dark, None),
            Appearance::Dark
        );
    }

    #[test]
    fn legible_on_preserves_a_preferred_color_that_clears_the_floor() {
        let theme = TuiTheme::default_for(Appearance::Dark);
        let preferred = Color::Rgb(255, 255, 255);

        assert_eq!(theme.legible_on(Color::Rgb(0, 0, 0), preferred), preferred);
    }

    #[test]
    fn terminal_secondary_pills_inherit_the_terminal_background() {
        assert_eq!(inherited_color(ThemeColor::Terminal), Color::Reset);
        assert_eq!(
            inherited_color(ThemeColor::Rgb(1, 2, 3)),
            Color::Rgb(1, 2, 3)
        );
    }

    #[test]
    fn legible_on_replaces_a_preferred_color_that_misses_the_floor() {
        let theme = TuiTheme::default_for(Appearance::Dark);
        let background = Color::Rgb(255, 255, 255);
        let result = theme.legible_on(background, Color::Rgb(240, 240, 240));

        assert!(contrast(result, background).unwrap() >= 4.5);
    }

    #[test]
    fn legible_on_clears_the_floor_for_every_built_in_theme() {
        for (name, _) in crate::theme::builtin::THEMES {
            let theme = TuiTheme::from(&crate::theme::load(name).unwrap());
            for (label, background, preferred) in [
                ("pill key", theme.pill_primary, theme.selection_fg),
                ("bar foreground", theme.bar_bg, theme.bar_fg),
                ("retained selection", theme.retained_bg, theme.accent),
                ("selected row", theme.selection_bg, theme.selection_fg),
                ("selected accent", theme.selection_bg, theme.accent),
                ("selected accent_alt", theme.selection_bg, theme.accent_alt),
                ("selected muted", theme.selection_bg, theme.muted),
                ("pill breadcrumb", theme.pill_secondary, theme.bar_fg),
                ("pill action", theme.pill_secondary, theme.pill_primary),
                ("pill trash action", theme.pill_secondary, theme.accent_alt),
                ("pill muted", theme.pill_secondary, theme.muted),
                ("pill rule", theme.pill_secondary, theme.rule),
                ("pill tag", theme.pill_secondary, theme.tag),
                ("pill success", theme.pill_secondary, theme.success),
                ("pill warning", theme.pill_secondary, theme.warning),
                ("pill error", theme.pill_secondary, theme.error),
                ("trash primary", theme.accent_alt, theme.selection_fg),
                ("search primary", theme.warning, theme.selection_fg),
            ] {
                let Some(_) = luminance(background) else {
                    continue;
                };
                let result = theme.legible_on(background, preferred);
                assert!(
                    contrast(result, background).unwrap() >= 4.5,
                    "theme {name} {label}: {result:?} is not legible on {background:?}"
                );
            }
        }
    }
}
