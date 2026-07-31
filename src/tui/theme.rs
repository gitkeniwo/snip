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
                if let Some(value) = overrides.get(stringify!($field)).and_then(toml::Value::as_str)
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
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn retained_selection(self) -> Style {
        Style::default()
            .fg(self.accent)
            .bg(self.retained_bg)
            .add_modifier(Modifier::BOLD)
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
            pill_secondary: color(ui.pill_secondary),
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
}
