use std::{fmt, str::FromStr};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Chord {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl Chord {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        normalize(code, modifiers)
    }

    pub fn from_event(event: KeyEvent) -> Option<Self> {
        (event.kind == KeyEventKind::Press).then(|| Self::new(event.code, event.modifiers))
    }

    pub fn code(self) -> KeyCode {
        self.code
    }

    pub fn modifiers(self) -> KeyModifiers {
        self.modifiers
    }

    pub fn canonical(self) -> String {
        self.render(Rendering::Canonical)
    }

    pub fn display(self) -> String {
        self.render(Rendering::Display)
    }

    pub fn compact(self) -> String {
        if self.modifiers == KeyModifiers::CONTROL {
            return format!("^{}", key_name(self.code, Rendering::Compact));
        }
        self.render(Rendering::Compact)
    }

    fn render(self, rendering: Rendering) -> String {
        let mut parts = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push(rendering.modifier("ctrl"));
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push(rendering.modifier("alt"));
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push(rendering.modifier("shift"));
        }
        parts.push(key_name(self.code, rendering));
        parts.join("-")
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.canonical())
    }
}

impl FromStr for Chord {
    type Err = ParseChordError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        if source.is_empty() {
            return Err(ParseChordError::new(source, "chord cannot be empty"));
        }
        if source.chars().any(char::is_whitespace) {
            return Err(ParseChordError::new(
                source,
                "chords cannot contain whitespace",
            ));
        }
        if source == "-" {
            return Ok(Self::new(KeyCode::Char('-'), KeyModifiers::NONE));
        }

        let parts: Vec<_> = source.split('-').collect();
        if parts.iter().any(|part| part.is_empty()) {
            return Err(ParseChordError::new(source, "invalid '-' placement"));
        }
        let (key, modifier_parts) = parts
            .split_last()
            .expect("the empty chord was rejected above");
        let mut modifiers = KeyModifiers::NONE;
        for modifier in modifier_parts {
            let parsed = match modifier.to_ascii_lowercase().as_str() {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                _ => {
                    return Err(ParseChordError::new(
                        source,
                        format!("unknown modifier {modifier:?}"),
                    ));
                }
            };
            if modifiers.contains(parsed) {
                return Err(ParseChordError::new(
                    source,
                    format!("duplicate modifier {modifier:?}"),
                ));
            }
            modifiers.insert(parsed);
        }

        let code = parse_key(key)
            .ok_or_else(|| ParseChordError::new(source, format!("unknown key {key:?}")))?;
        Ok(Self::new(code, modifiers))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseChordError {
    chord: String,
    reason: String,
}

impl ParseChordError {
    fn new(chord: &str, reason: impl Into<String>) -> Self {
        Self {
            chord: chord.to_owned(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ParseChordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid chord {:?}: {}", self.chord, self.reason)
    }
}

impl std::error::Error for ParseChordError {}

#[derive(Clone, Copy)]
enum Rendering {
    Canonical,
    Display,
    Compact,
}

impl Rendering {
    fn modifier(self, name: &'static str) -> String {
        match self {
            Self::Canonical => name.to_owned(),
            Self::Display | Self::Compact => {
                let mut chars = name.chars();
                let first = chars.next().expect("modifier names are non-empty");
                format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
            }
        }
    }
}

fn normalize(mut code: KeyCode, mut modifiers: KeyModifiers) -> Chord {
    if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        code = KeyCode::BackTab;
        modifiers.remove(KeyModifiers::SHIFT);
    }
    if code == KeyCode::BackTab {
        modifiers.remove(KeyModifiers::SHIFT);
    }
    if let KeyCode::Char(character) = code {
        if modifiers.contains(KeyModifiers::CONTROL) {
            code = KeyCode::Char(character.to_ascii_lowercase());
            modifiers.remove(KeyModifiers::SHIFT);
        } else if modifiers.contains(KeyModifiers::SHIFT) {
            code = KeyCode::Char(character.to_ascii_uppercase());
            modifiers.remove(KeyModifiers::SHIFT);
        }
    }
    Chord { code, modifiers }
}

fn parse_key(key: &str) -> Option<KeyCode> {
    let lower = key.to_ascii_lowercase();
    let named = match lower.as_str() {
        "enter" => Some(KeyCode::Enter),
        "esc" => Some(KeyCode::Esc),
        "tab" => Some(KeyCode::Tab),
        "backtab" => Some(KeyCode::BackTab),
        "space" => Some(KeyCode::Char(' ')),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "insert" => Some(KeyCode::Insert),
        "delete" => Some(KeyCode::Delete),
        _ => lower
            .strip_prefix('f')
            .and_then(|number| number.parse::<u8>().ok())
            .filter(|number| (1..=12).contains(number))
            .map(KeyCode::F),
    };
    named.or_else(|| {
        let mut chars = key.chars();
        let character = chars.next()?;
        chars.next().is_none().then_some(KeyCode::Char(character))
    })
}

fn key_name(code: KeyCode, rendering: Rendering) -> String {
    let canonical = matches!(rendering, Rendering::Canonical);
    match code {
        KeyCode::Char(' ') => if canonical { "space" } else { "Space" }.to_owned(),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Enter => if canonical { "enter" } else { "Enter" }.to_owned(),
        KeyCode::Esc => if canonical { "esc" } else { "Esc" }.to_owned(),
        KeyCode::Tab => if canonical { "tab" } else { "Tab" }.to_owned(),
        KeyCode::BackTab => if canonical { "backtab" } else { "Shift-Tab" }.to_owned(),
        KeyCode::Up => if canonical { "up" } else { "Up" }.to_owned(),
        KeyCode::Down => if canonical { "down" } else { "Down" }.to_owned(),
        KeyCode::Left => if canonical { "left" } else { "Left" }.to_owned(),
        KeyCode::Right => if canonical { "right" } else { "Right" }.to_owned(),
        KeyCode::Home => if canonical { "home" } else { "Home" }.to_owned(),
        KeyCode::End => if canonical { "end" } else { "End" }.to_owned(),
        KeyCode::PageUp => if canonical { "pageup" } else { "PageUp" }.to_owned(),
        KeyCode::PageDown => if canonical { "pagedown" } else { "PageDown" }.to_owned(),
        KeyCode::Insert => if canonical { "insert" } else { "Insert" }.to_owned(),
        KeyCode::Delete => if canonical { "delete" } else { "Delete" }.to_owned(),
        KeyCode::F(number) => format!("{}{}", if canonical { "f" } else { "F" }, number),
        unsupported => panic!("unsupported key code in chord: {unsupported:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_characters_normalize_to_case() {
        let upper: Chord = "J".parse().unwrap();
        let shifted: Chord = "shift-j".parse().unwrap();
        let event = Chord::from_event(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));

        assert_eq!(upper, shifted);
        assert_eq!(event, Some(upper));
        assert_eq!(upper.canonical(), "J");
    }

    #[test]
    fn control_characters_are_case_insensitive() {
        let lower: Chord = "ctrl-p".parse().unwrap();
        assert_eq!("ctrl-D".parse::<Chord>().unwrap().canonical(), "ctrl-d");
        assert_eq!("Ctrl-d".parse::<Chord>().unwrap().canonical(), "ctrl-d");
        assert_eq!("ctrl-shift-p".parse::<Chord>().unwrap(), lower);
        assert_eq!(
            Chord::from_event(KeyEvent::new(
                KeyCode::Char('P'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            )),
            Some(lower)
        );
    }

    #[test]
    fn backtab_has_one_canonical_form() {
        let backtab: Chord = "backtab".parse().unwrap();
        assert_eq!("shift-tab".parse::<Chord>().unwrap(), backtab);
        assert_eq!(
            Chord::from_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            Some(backtab)
        );
        assert_eq!(backtab.canonical(), "backtab");
    }

    #[test]
    fn rejects_spaces_and_future_sequences() {
        assert!(" ".parse::<Chord>().is_err());
        assert!("g d".parse::<Chord>().is_err());
        assert!("ctrl- g".parse::<Chord>().is_err());
    }

    #[test]
    fn renders_config_help_and_bottom_bar_forms() {
        let chord: Chord = "ctrl-alt-t".parse().unwrap();
        assert_eq!(chord.canonical(), "ctrl-alt-t");
        assert_eq!(chord.display(), "Ctrl-Alt-t");
        assert_eq!(chord.compact(), "Ctrl-Alt-t");

        let control: Chord = "ctrl-g".parse().unwrap();
        assert_eq!(control.compact(), "^g");
    }
}
