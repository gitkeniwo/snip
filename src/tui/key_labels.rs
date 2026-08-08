use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::keys::{Chord, Keymap, Mode};
use crate::tui::command::CommandId;

pub(crate) fn effective_chords(
    keymap: &Keymap,
    stack: &[Mode],
    bindings: &[(Mode, CommandId)],
) -> Vec<Chord> {
    let mut chords = Vec::new();
    for &binding @ (mode, command) in bindings {
        for chord in keymap.chords_for(&[mode], command) {
            if keymap.resolve_with_mode(stack, chord) == Some(binding) && !chords.contains(&chord) {
                chords.push(chord);
            }
        }
    }
    chords
}

pub(crate) fn display_primary_bindings(
    keymap: &Keymap,
    stack: &[Mode],
    bindings: &[(Mode, CommandId)],
) -> String {
    let mut chords = Vec::new();
    for &binding in bindings {
        if let Some(chord) = effective_chords(keymap, stack, &[binding])
            .into_iter()
            .next()
            && !chords.contains(&chord)
        {
            chords.push(chord);
        }
    }
    chords
        .into_iter()
        .map(display_chord)
        .collect::<Vec<_>>()
        .join(" / ")
}

pub(crate) fn display_chord(chord: Chord) -> String {
    if chord.modifiers() == KeyModifiers::NONE {
        match chord.code() {
            KeyCode::Up => return "↑".to_owned(),
            KeyCode::Down => return "↓".to_owned(),
            KeyCode::Left => return "←".to_owned(),
            KeyCode::Right => return "→".to_owned(),
            _ => {}
        }
    }
    chord.display()
}

pub(crate) fn compact_chord(chord: Chord) -> String {
    if chord.modifiers() == KeyModifiers::NONE {
        match chord.code() {
            KeyCode::Up => return "↑".to_owned(),
            KeyCode::Down => return "↓".to_owned(),
            KeyCode::Left => return "←".to_owned(),
            KeyCode::Right => return "→".to_owned(),
            _ => {}
        }
    }
    chord.compact()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_labels_drop_global_bindings_shadowed_by_an_exclusive_mode() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(&path, "[global]\n\"app.quit\" = \"x\"\n").unwrap();
        let (keymap, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());

        assert!(
            display_primary_bindings(
                &keymap,
                &[Mode::Trash],
                &[(Mode::Global, CommandId::AppQuit)]
            )
            .is_empty()
        );
        assert_eq!(
            display_primary_bindings(
                &keymap,
                &[Mode::Trash],
                &[(Mode::Trash, CommandId::TrashPurgeSelected)]
            ),
            "x"
        );
    }

    #[test]
    fn effective_labels_distinguish_the_same_command_in_different_modes() {
        let keymap = Keymap::defaults();

        assert!(
            display_primary_bindings(
                &keymap,
                &[Mode::Sidebar, Mode::Global],
                &[(Mode::Trash, CommandId::NavDown)]
            )
            .is_empty()
        );
        assert_eq!(
            display_primary_bindings(
                &keymap,
                &[Mode::Sidebar, Mode::Global],
                &[(Mode::Sidebar, CommandId::NavDown)]
            ),
            "j"
        );
    }
}
