use std::collections::BTreeMap;

use snip::keys::{Chord, Keymap, Mode, chord_sort_key};
use snip::tui::command::{self, CommandId};
use snip::tui::help::{EXTRAS, HelpExtraGroup};

pub struct KeyDoc {
    pub modes: Vec<ModeDoc>,
    pub mouse: Vec<MouseDoc>,
}

pub struct ModeDoc {
    pub label: &'static str,
    pub blurb: &'static str,
    pub rows: Vec<KeyRow>,
    pub inherited: Vec<KeyRow>,
}

pub struct KeyRow {
    pub keys: Vec<String>,
    pub action: Option<&'static str>,
    pub description: &'static str,
}

pub struct MouseDoc {
    pub key: &'static str,
    pub description: &'static str,
    pub modes: Vec<&'static str>,
}

pub fn collect(keymap: &Keymap) -> KeyDoc {
    let modes = Mode::ALL
        .into_iter()
        .map(|mode| collect_mode(keymap, mode))
        .collect();
    let mouse = EXTRAS
        .iter()
        .filter(|extra| extra.group == HelpExtraGroup::Mouse)
        .map(|extra| MouseDoc {
            key: extra.key,
            description: extra.description,
            modes: extra.modes.iter().map(|mode| mode_label(*mode)).collect(),
        })
        .collect();
    KeyDoc { modes, mouse }
}

fn collect_mode(keymap: &Keymap, mode: Mode) -> ModeDoc {
    let mut by_command: BTreeMap<usize, (CommandId, Vec<Chord>)> = BTreeMap::new();
    for (chord, id) in keymap.bindings_for(mode) {
        let order = CommandId::ALL
            .iter()
            .position(|candidate| *candidate == id)
            .expect("every bound command is registered");
        by_command
            .entry(order)
            .or_insert_with(|| (id, Vec::new()))
            .1
            .push(chord);
    }

    let mut bound_rows = by_command
        .into_values()
        .map(|(id, mut chords)| {
            chords.sort_by_key(|chord| chord_sort_key(*chord));
            (id, chords)
        })
        .collect::<Vec<_>>();
    bound_rows.sort_by(|left, right| {
        chord_sort_key(left.1[0])
            .cmp(&chord_sort_key(right.1[0]))
            .then(command::get(left.0).slug.cmp(command::get(right.0).slug))
    });
    let mut rows = bound_rows
        .into_iter()
        .map(|(id, chords)| {
            let command = command::get(id);
            KeyRow {
                keys: chords.into_iter().map(|chord| chord.display()).collect(),
                action: Some(command.slug),
                description: command.description,
            }
        })
        .collect::<Vec<_>>();
    rows.extend(
        EXTRAS
            .iter()
            .filter(|extra| extra.modes.contains(&mode))
            .map(|extra| KeyRow {
                keys: vec![extra.key.to_owned()],
                action: (!extra.slug.is_empty()).then_some(extra.slug),
                description: extra.description,
            }),
    );

    let inherited = mode
        .inherits()
        .iter()
        .map(|id| {
            let command = command::get(*id);
            KeyRow {
                keys: keymap
                    .chords_for(&[Mode::Global], *id)
                    .iter()
                    .map(|chord| chord.display())
                    .collect(),
                action: Some(command.slug),
                description: command.description,
            }
        })
        .collect();

    ModeDoc {
        label: mode_label(mode),
        blurb: mode_blurb(mode),
        rows,
        inherited,
    }
}

pub const fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Global => "global",
        Mode::Sidebar => "sidebar",
        Mode::List => "list",
        Mode::Preview => "preview",
        Mode::Fragment => "fragment",
        Mode::FragmentGrab => "fragment-grab",
        Mode::Trash => "trash",
        Mode::Help => "help",
        Mode::Git => "git",
        Mode::Gist => "gist",
        Mode::Search => "search",
    }
}

pub const fn mode_blurb(mode: Mode) -> &'static str {
    match mode {
        Mode::Global => {
            "Part of normal pane stacks; exclusive modes inherit only commands in their allowlist."
        }
        Mode::Sidebar => "Live while the library pane on the left has focus.",
        Mode::List => "Live while the snippet list has focus.",
        Mode::Preview => "Live while the preview pane has focus.",
        Mode::Fragment => "Live while the preview has focus and the fragment list is expanded.",
        Mode::FragmentGrab => "Live while a fragment is grabbed and waiting to be dropped.",
        Mode::Trash => "Live while the trash pane has focus. Takes over the keyboard.",
        Mode::Help => "Live while this help panel is open. Takes over the keyboard.",
        Mode::Git => "Live while the Git console is open. Takes over the keyboard.",
        Mode::Gist => "Live while the Gist panel is open. Takes over the keyboard.",
        Mode::Search => "Live while a search query is being typed. Takes over the keyboard.",
    }
}
