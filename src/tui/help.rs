mod extras;
mod render;
mod state;

use std::borrow::Cow;
use std::collections::HashSet;

use crate::keys::{Chord, Keymap, Mode, chord_sort_key};
use crate::tui::command::{self, CommandId};

pub use extras::{EXTRAS, HelpExtra, HelpExtraGroup};
pub use render::{HelpRenderPlan, WrappedLine, build_render_plan, draw_help, wrap_words};
pub use state::{HelpMatch, HelpState, HiddenMatch};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HelpRowId {
    Command { source_mode: Mode, id: CommandId },
    Extra(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HelpKey {
    Chords(Vec<Chord>),
    Literal(&'static str),
}

impl HelpKey {
    pub fn display(&self) -> String {
        match self {
            Self::Chords(chords) => chords
                .iter()
                .copied()
                .map(help_chord)
                .collect::<Vec<_>>()
                .join(" / "),
            Self::Literal(label) => (*label).to_owned(),
        }
    }

    fn first_chord(&self) -> Option<Chord> {
        match self {
            Self::Chords(chords) => chords.first().copied(),
            Self::Literal(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HelpGroup {
    Mode(Mode),
    Inherited,
    HelpControls,
    Numbers,
    Mouse,
    System,
}

impl HelpGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mode(Mode::Global) => "GLOBAL",
            Self::Mode(Mode::Sidebar) => "SIDEBAR — LEFT PANE",
            Self::Mode(Mode::List) => "SNIPPET LIST",
            Self::Mode(Mode::Preview) => "PREVIEW",
            Self::Mode(Mode::Fragment) => "FRAGMENTS",
            Self::Mode(Mode::FragmentGrab) => "FRAGMENT MOVE",
            Self::Mode(Mode::Trash) => "TRASH",
            Self::Mode(Mode::Git) => "GIT CONSOLE",
            Self::Mode(Mode::Gist) => "GIST PANEL",
            Self::Mode(Mode::Search) => "SEARCH",
            Self::Mode(Mode::Help) => "HELP",
            Self::Inherited => "INHERITED FROM GLOBAL",
            Self::HelpControls => "HELP CONTROLS",
            Self::Numbers => "NUMBERS",
            Self::Mouse => "MOUSE",
            Self::System => "SYSTEM",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpScope {
    Context,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpSort {
    Key,
    Action,
}

#[derive(Clone, Debug)]
pub struct HelpRow {
    pub id: HelpRowId,
    pub display_group: HelpGroup,
    pub source_mode: Option<Mode>,
    pub key: HelpKey,
    pub slug: &'static str,
    pub description: &'static str,
    pub title: Cow<'static, str>,
    pub category: &'static str,
    pub keywords: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub user_modified: bool,
    extra_order: usize,
}

#[derive(Clone, Debug)]
pub struct VisibleHelpRow {
    pub row: HelpRow,
    pub matched: HelpMatch,
}

pub fn declared(mode: Mode, keymap: &Keymap, gui_editor: Option<&str>) -> Vec<HelpRow> {
    let defaults = Keymap::defaults();
    let mut bindings = keymap.bindings_for(mode).collect::<Vec<_>>();
    bindings.sort_unstable_by_key(|(chord, _)| chord_sort_key(*chord));
    let mut rows = Vec::<HelpRow>::new();
    for (chord, id) in bindings {
        if let Some(row) = rows.iter_mut().find(|row| {
            row.id
                == HelpRowId::Command {
                    source_mode: mode,
                    id,
                }
        }) {
            if let HelpKey::Chords(chords) = &mut row.key {
                chords.push(chord);
            }
            continue;
        }
        rows.push(command_row(
            mode,
            HelpGroup::Mode(mode),
            id,
            vec![chord],
            keymap,
            &defaults,
            gui_editor,
        ));
    }
    rows
}

pub fn effective(stack: &[Mode], keymap: &Keymap, gui_editor: Option<&str>) -> Vec<HelpRow> {
    let defaults = Keymap::defaults();
    let mut candidates = stack
        .iter()
        .flat_map(|mode| keymap.bindings_for(*mode).map(|(chord, _)| chord))
        .collect::<HashSet<_>>();
    if let Some(head) = stack.first().copied().filter(|mode| mode.is_exclusive()) {
        for (chord, id) in keymap.bindings_for(Mode::Global) {
            if head.inherits().contains(&id) {
                candidates.insert(chord);
            }
        }
    }
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|chord| chord_sort_key(*chord));
    let mut rows = Vec::<HelpRow>::new();
    for chord in candidates {
        let Some((source_mode, id)) = keymap.resolve_with_mode(stack, chord) else {
            continue;
        };
        let display_group = if source_mode == Mode::Global && !stack.contains(&Mode::Global) {
            HelpGroup::Inherited
        } else {
            HelpGroup::Mode(source_mode)
        };
        let row_id = HelpRowId::Command { source_mode, id };
        if let Some(row) = rows.iter_mut().find(|row| row.id == row_id) {
            if let HelpKey::Chords(chords) = &mut row.key {
                chords.push(chord);
            }
        } else {
            rows.push(command_row(
                source_mode,
                display_group,
                id,
                vec![chord],
                keymap,
                &defaults,
                gui_editor,
            ));
        }
    }
    rows
}

pub(crate) fn project(
    scope: HelpScope,
    stack: &[Mode],
    keymap: &Keymap,
    gui_editor: Option<&str>,
) -> Vec<HelpRow> {
    let mut rows = match scope {
        HelpScope::Context => effective(stack, keymap, gui_editor),
        HelpScope::All => Mode::ALL
            .into_iter()
            .flat_map(|mode| declared(mode, keymap, gui_editor))
            .collect(),
    };
    let applicable_modes: &[Mode] = match scope {
        HelpScope::Context => stack,
        HelpScope::All => &Mode::ALL,
    };
    let mut extra_ids = HashSet::new();
    for (order, extra) in EXTRAS.iter().enumerate() {
        if extra
            .modes
            .iter()
            .any(|mode| applicable_modes.contains(mode))
            && extra_ids.insert(extra.id)
        {
            rows.push(extra_row(extra, order));
        }
    }
    if scope == HelpScope::Context {
        for mut row in declared(Mode::Help, keymap, gui_editor) {
            row.display_group = HelpGroup::HelpControls;
            rows.push(row);
        }
        for (order, extra) in EXTRAS.iter().enumerate() {
            if extra.group == HelpExtraGroup::HelpControls && extra_ids.insert(extra.id) {
                rows.push(extra_row(extra, order));
            }
        }
    }
    rows
}

pub(crate) fn sort_rows(rows: &mut [HelpRow], sort: HelpSort, stack: &[Mode]) {
    rows.sort_by(|left, right| {
        group_rank(left.display_group, stack)
            .cmp(&group_rank(right.display_group, stack))
            .then_with(|| match (&left.id, &right.id) {
                (HelpRowId::Extra(_), HelpRowId::Extra(_)) => {
                    left.extra_order.cmp(&right.extra_order)
                }
                _ => match sort {
                    HelpSort::Key => left
                        .key
                        .first_chord()
                        .map(chord_sort_key)
                        .cmp(&right.key.first_chord().map(chord_sort_key))
                        .then(left.slug.cmp(right.slug)),
                    HelpSort::Action => left.slug.cmp(right.slug),
                },
            })
    });
}

fn group_rank(group: HelpGroup, stack: &[Mode]) -> (u8, usize) {
    const CONTEXT: u8 = 0;
    const OTHER_MODES: u8 = 1;
    const EXTRAS: u8 = 2;
    const HELP_CONTROLS: u8 = 3;

    match group {
        HelpGroup::Mode(mode) => stack
            .iter()
            .position(|candidate| *candidate == mode)
            .map_or_else(
                || {
                    (
                        OTHER_MODES,
                        Mode::ALL
                            .iter()
                            .position(|candidate| *candidate == mode)
                            .unwrap_or(Mode::ALL.len()),
                    )
                },
                |index| (CONTEXT, index),
            ),
        HelpGroup::Inherited => (CONTEXT, stack.len()),
        HelpGroup::Numbers => (EXTRAS, 0),
        HelpGroup::Mouse => (EXTRAS, 1),
        HelpGroup::System => (EXTRAS, 2),
        HelpGroup::HelpControls => (HELP_CONTROLS, 0),
    }
}

fn command_row(
    source_mode: Mode,
    display_group: HelpGroup,
    id: CommandId,
    chords: Vec<Chord>,
    keymap: &Keymap,
    defaults: &Keymap,
    gui_editor: Option<&str>,
) -> HelpRow {
    let command = command::get(id);
    HelpRow {
        id: HelpRowId::Command { source_mode, id },
        display_group,
        source_mode: Some(source_mode),
        key: HelpKey::Chords(chords),
        slug: command.slug,
        description: command.description,
        title: command::display_title(command, gui_editor),
        category: command.category,
        keywords: command.keywords,
        aliases: &[],
        user_modified: keymap.chords_for(&[source_mode], id)
            != defaults.chords_for(&[source_mode], id),
        extra_order: 0,
    }
}

fn extra_row(extra: &'static HelpExtra, extra_order: usize) -> HelpRow {
    let display_group = match extra.group {
        HelpExtraGroup::Mode(mode) => HelpGroup::Mode(mode),
        HelpExtraGroup::HelpControls => HelpGroup::HelpControls,
        HelpExtraGroup::Numbers => HelpGroup::Numbers,
        HelpExtraGroup::Mouse => HelpGroup::Mouse,
        HelpExtraGroup::System => HelpGroup::System,
    };
    HelpRow {
        id: HelpRowId::Extra(extra.id),
        display_group,
        source_mode: None,
        key: HelpKey::Literal(extra.key),
        slug: extra.slug,
        description: extra.description,
        title: Cow::Borrowed(""),
        category: "",
        keywords: &[],
        aliases: extra.aliases,
        user_modified: false,
        extra_order,
    }
}

pub fn help_chord(chord: Chord) -> String {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_rows_drop_shadowed_bindings() {
        let keymap = Keymap::defaults();
        let rows = effective(&[Mode::List, Mode::Global], &keymap, None);
        let enter = rows
            .iter()
            .filter(|row| row.key.display().contains("Enter"))
            .collect::<Vec<_>>();
        assert_eq!(enter.len(), 1);
        assert_eq!(enter[0].source_mode, Some(Mode::List));
    }

    #[test]
    fn effective_rows_surface_only_inherited_globals() {
        let keymap = Keymap::defaults();
        let rows = effective(&[Mode::Help], &keymap, None);
        assert!(rows.iter().any(|row| row.slug == "git.toggle-console"));
        assert!(rows.iter().any(|row| row.slug == "gist.toggle-panel"));
        assert!(rows.iter().any(|row| row.slug == "palette.open"));
        assert!(!rows.iter().any(|row| row.slug == "copy.content"));
        assert!(
            rows.iter()
                .filter(|row| row.display_group == HelpGroup::Inherited)
                .count()
                >= 3
        );
    }

    #[test]
    fn context_appends_help_controls_once_and_all_does_not_duplicate() {
        let keymap = Keymap::defaults();
        let context = project(
            HelpScope::Context,
            &[Mode::List, Mode::Global],
            &keymap,
            None,
        );
        assert!(
            context
                .iter()
                .any(|row| row.display_group == HelpGroup::HelpControls)
        );
        let all = project(HelpScope::All, &[Mode::List, Mode::Global], &keymap, None);
        assert_eq!(
            all.iter()
                .filter(|row| row.slug == "help.toggle-scope")
                .count(),
            1
        );
    }

    #[test]
    fn help_rows_use_the_configured_gui_editor_name() {
        let rows = declared(Mode::List, &Keymap::defaults(), Some("zed"));
        let open = rows
            .iter()
            .find(|row| row.slug == "snippet.open-gui")
            .unwrap();
        assert_eq!(open.title, "Open in zed");
    }
}
