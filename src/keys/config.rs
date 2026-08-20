use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::error::{Result, SnipError};
use crate::tui::command::{CommandId, get, resolve_slug};

use super::{Chord, Keymap, Mode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticLevel {
    Error,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

impl Diagnostic {
    fn error(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: message.into(),
        }
    }

    fn info(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Info,
            message: message.into(),
        }
    }
}

struct UserBinding {
    id: CommandId,
    slug: String,
    chords: Vec<Chord>,
}

impl Keymap {
    /// Load the user keymap, or return the built-in bindings when the file is absent.
    pub fn load() -> Result<(Self, Vec<Diagnostic>)> {
        let path = path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<(Self, Vec<Diagnostic>)> {
        if !path.exists() {
            return Ok((Self::defaults(), Vec::new()));
        }
        let text = fs::read_to_string(path).map_err(|error| {
            SnipError::io(format!(
                "cannot read key bindings {}: {error}",
                path.display()
            ))
        })?;
        let table = toml::from_str::<toml::Table>(&text).map_err(|error| {
            SnipError::validation(format!(
                "cannot parse key bindings {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self::from_config_table(table))
    }

    fn from_config_table(mut table: toml::Table) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let inherit_defaults = match table.remove("inherit-defaults") {
            None => true,
            Some(toml::Value::Boolean(value)) => value,
            Some(_) => {
                diagnostics.push(Diagnostic::error(
                    "inherit-defaults must be true or false; using true",
                ));
                true
            }
        };
        let mut keymap = if inherit_defaults {
            Self::defaults()
        } else {
            Self::empty()
        };

        for (mode_name, value) in table {
            let Some(mode) = Mode::from_config_name(&mode_name) else {
                diagnostics.push(Diagnostic::error(format!(
                    "unknown key binding mode \"{mode_name}\""
                )));
                continue;
            };
            if mode == Mode::Search {
                diagnostics.push(Diagnostic::error("mode \"search\" cannot be configured"));
                continue;
            }
            let toml::Value::Table(bindings) = value else {
                diagnostics.push(Diagnostic::error(format!(
                    "key binding mode \"{mode_name}\" must be a table"
                )));
                continue;
            };
            let user_bindings = parse_mode(mode, bindings, &mut diagnostics);
            keymap.merge_mode(mode, user_bindings, &mut diagnostics);
        }

        (keymap, diagnostics)
    }

    fn merge_mode(
        &mut self,
        mode: Mode,
        user_bindings: Vec<UserBinding>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut claims: HashMap<Chord, Vec<usize>> = HashMap::new();
        for (index, binding) in user_bindings.iter().enumerate() {
            for &chord in &binding.chords {
                claims.entry(chord).or_default().push(index);
            }
        }

        let mut rejected = HashSet::new();
        for (chord, claimants) in &claims {
            if claimants.len() < 2 {
                continue;
            }
            rejected.extend(claimants.iter().copied());
            let slugs = claimants
                .iter()
                .map(|&index| user_bindings[index].slug.as_str())
                .collect::<Vec<_>>();
            let claim = if let [first, second] = slugs.as_slice() {
                format!("both {first} and {second}")
            } else {
                slugs.join(", ")
            };
            diagnostics.push(Diagnostic::error(format!(
                "{chord} claimed by {claim} in mode \"{}\"",
                mode.config_name()
            )));
        }

        // Remove every mentioned action before inserting any replacement. This
        // is what makes exchanging two default chords a clean operation.
        let bindings = self.modes.entry(mode).or_default();
        let previous_bindings = bindings.clone();
        for binding in &user_bindings {
            bindings.retain(|_, id| *id != binding.id);
        }
        // A contradictory pair is ignored as a unit, so restore the bindings
        // each action had before this mode's merge.
        for &index in &rejected {
            let id = user_bindings[index].id;
            bindings.extend(
                previous_bindings
                    .iter()
                    .filter(|(_, previous)| **previous == id)
                    .map(|(chord, previous)| (*chord, *previous)),
            );
        }

        for (index, binding) in user_bindings.into_iter().enumerate() {
            if rejected.contains(&index) {
                continue;
            }
            for chord in binding.chords {
                if let Some(previous) = bindings.insert(chord, binding.id)
                    && previous != binding.id
                {
                    let previous_slug = get(previous).slug;
                    let has_other_key = bindings.values().any(|id| *id == previous);
                    let suffix = if has_other_key {
                        format!(" in mode \"{}\"", mode.config_name())
                    } else {
                        format!(
                            "; {previous_slug} now has no key in mode \"{}\"",
                            mode.config_name()
                        )
                    };
                    diagnostics.push(Diagnostic::info(format!(
                        "{chord} taken from {previous_slug}{suffix}"
                    )));
                }
            }
        }
    }
}

pub fn path() -> Result<std::path::PathBuf> {
    Ok(crate::config::config_path()?.with_file_name("keys.toml"))
}

fn parse_mode(
    mode: Mode,
    bindings: toml::Table,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<UserBinding> {
    let reserved = "ctrl-c".parse::<Chord>().expect("reserved chord is valid");
    let mut parsed = Vec::new();

    for (slug, value) in bindings {
        let Some((id, replacement)) = resolve_slug(&slug) else {
            diagnostics.push(Diagnostic::error(format!(
                "unknown key binding action \"{slug}\" in mode \"{}\"",
                mode.config_name()
            )));
            continue;
        };
        if let Some(replacement) = replacement {
            diagnostics.push(Diagnostic::info(format!(
                "key binding action \"{slug}\" is deprecated; use \"{replacement}\""
            )));
        }
        let Some(chord_names) = chord_names(&value) else {
            diagnostics.push(Diagnostic::error(format!(
                "binding for {slug} in mode \"{}\" must be a chord or a list of chords",
                mode.config_name()
            )));
            continue;
        };

        let explicitly_empty = chord_names.is_empty();
        let mut chords = Vec::new();
        for chord_name in chord_names {
            match chord_name.parse::<Chord>() {
                Ok(chord) if chord == reserved => diagnostics.push(Diagnostic::error(format!(
                    "ctrl-c is reserved and cannot be bound to {slug} in mode \"{}\"",
                    mode.config_name()
                ))),
                Ok(chord) => chords.push(chord),
                Err(error) => diagnostics.push(Diagnostic::error(format!(
                    "invalid chord \"{chord_name}\" for {slug} in mode \"{}\": {error}",
                    mode.config_name()
                ))),
            }
        }
        chords.sort_unstable_by_key(|chord| chord.canonical());
        chords.dedup();
        // A list made entirely of invalid chords is a typo, not an unbind. Only
        // an explicit empty list has replacement semantics with no new chords.
        if !chords.is_empty() || explicitly_empty {
            parsed.push(UserBinding { id, slug, chords });
        }
    }

    parsed
}

fn chord_names(value: &toml::Value) -> Option<Vec<&str>> {
    match value {
        toml::Value::String(chord) => Some(vec![chord]),
        toml::Value::Array(chords) => chords.iter().map(toml::Value::as_str).collect(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(text: &str) -> (Keymap, Vec<Diagnostic>) {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        fs::write(&path, text).unwrap();
        Keymap::load_from(&path).unwrap()
    }

    #[test]
    fn swaps_two_actions_after_removing_all_mentioned_defaults() {
        let (keymap, diagnostics) = load(
            r#"
                [list]
                "snippet.edit-content" = "r"
                "snippet.rename" = "e"
            "#,
        );

        assert!(diagnostics.is_empty());
        assert_eq!(
            keymap.resolve(&[Mode::List], "r".parse().unwrap()),
            Some(CommandId::SnippetEditContent)
        );
        assert_eq!(
            keymap.resolve(&[Mode::List], "e".parse().unwrap()),
            Some(CommandId::SnippetRename)
        );
    }

    #[test]
    fn rejects_both_user_actions_when_they_claim_one_chord() {
        let (keymap, diagnostics) = load(
            r#"
                [list]
                "snippet.edit-content" = "x"
                "snippet.rename" = "x"
            "#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, DiagnosticLevel::Error);
        assert_eq!(
            keymap.resolve(&[Mode::List], "e".parse().unwrap()),
            Some(CommandId::SnippetEditContent)
        );
        assert_eq!(
            keymap.resolve(&[Mode::List], "r".parse().unwrap()),
            Some(CommandId::SnippetRename)
        );
        assert_eq!(keymap.resolve(&[Mode::List], "x".parse().unwrap()), None);
    }

    #[test]
    fn user_binding_evicts_an_unmentioned_default_with_an_info_diagnostic() {
        let (keymap, diagnostics) = load(
            r#"
                [list]
                "snippet.edit-content" = "m"
            "#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, DiagnosticLevel::Info);
        assert_eq!(
            diagnostics[0].message,
            "m taken from snippet.move; snippet.move now has no key in mode \"list\""
        );
        assert_eq!(
            keymap.resolve(&[Mode::List], "m".parse().unwrap()),
            Some(CommandId::SnippetEditContent)
        );
        assert_eq!(keymap.resolve(&[Mode::List], "e".parse().unwrap()), None);
        assert!(
            !keymap
                .bindings_for(Mode::List)
                .any(|(_, id)| id == CommandId::SnippetMove)
        );
    }

    #[test]
    fn inherit_defaults_false_starts_from_an_empty_keymap() {
        let (keymap, diagnostics) = load(
            r#"
                inherit-defaults = false

                [global]
                "app.quit" = "x"
            "#,
        );

        assert!(diagnostics.is_empty());
        assert_eq!(
            keymap.resolve(&[Mode::Global], "x".parse().unwrap()),
            Some(CommandId::AppQuit)
        );
        assert_eq!(keymap.resolve(&[Mode::Global], "q".parse().unwrap()), None);
    }

    #[test]
    fn invalid_entries_are_diagnostics_and_do_not_discard_valid_defaults() {
        let (keymap, diagnostics) = load(
            r#"
                [search]
                "snippet.new" = "n"

                [list]
                "snippet.edit-content" = "g d"
                "missing.action" = "x"
                "snippet.rename" = "ctrl-c"
            "#,
        );

        assert_eq!(diagnostics.len(), 4);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.level == DiagnosticLevel::Error)
        );
        assert_eq!(
            keymap.resolve(&[Mode::List], "e".parse().unwrap()),
            Some(CommandId::SnippetEditContent)
        );
        assert_eq!(
            keymap.resolve(&[Mode::List], "r".parse().unwrap()),
            Some(CommandId::SnippetRename)
        );
    }

    #[test]
    fn deprecated_gui_editor_slug_binds_with_one_info_diagnostic() {
        let (keymap, diagnostics) = load(
            r#"
                [list]
                "snippet.open-vscode" = "alt-v"
            "#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, DiagnosticLevel::Info);
        assert_eq!(
            diagnostics[0].message,
            "key binding action \"snippet.open-vscode\" is deprecated; use \"snippet.open-gui\""
        );
        assert_eq!(
            keymap.resolve(&[Mode::List], "alt-v".parse().unwrap()),
            Some(CommandId::SnippetOpenGui)
        );

        let (_, canonical_diagnostics) = load(
            r#"
                [list]
                "snippet.open-gui" = "alt-v"
            "#,
        );
        assert!(canonical_diagnostics.is_empty());
    }
}
