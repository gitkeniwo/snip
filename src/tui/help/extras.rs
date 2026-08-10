use crate::keys::Mode;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HelpExtraGroup {
    Mode(Mode),
    HelpControls,
    Numbers,
    Mouse,
    System,
}

#[derive(Clone, Copy, Debug)]
pub struct HelpExtra {
    pub id: &'static str,
    pub modes: &'static [Mode],
    pub group: HelpExtraGroup,
    pub key: &'static str,
    pub slug: &'static str,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
}

pub const EXTRAS: &[HelpExtra] = &[
    HelpExtra {
        id: "system.force-quit",
        modes: &Mode::ALL,
        group: HelpExtraGroup::System,
        key: "Ctrl-c",
        slug: "app.quit",
        description: "Force quit before the active mode handles input",
        aliases: &["quit", "exit", "system"],
    },
    HelpExtra {
        id: "numbers.jump",
        modes: &[Mode::Sidebar, Mode::List, Mode::Preview, Mode::Fragment],
        group: HelpExtraGroup::Numbers,
        key: "1-9 / 0",
        slug: "",
        description: "Jump to the first through tenth item",
        aliases: &["digits", "number", "shortcut"],
    },
    HelpExtra {
        id: "search.type",
        modes: &[Mode::Search],
        group: HelpExtraGroup::Mode(Mode::Search),
        key: "any printable character",
        slug: "",
        description: "Append a character to the search query",
        aliases: &["type", "input", "query"],
    },
    HelpExtra {
        id: "search.backspace",
        modes: &[Mode::Search],
        group: HelpExtraGroup::Mode(Mode::Search),
        key: "Backspace",
        slug: "",
        description: "Remove the final character from the search query",
        aliases: &["delete", "erase"],
    },
    HelpExtra {
        id: "search.enter",
        modes: &[Mode::Search],
        group: HelpExtraGroup::Mode(Mode::Search),
        key: "Enter",
        slug: "",
        description: "Keep the query and return focus to the snippet list",
        aliases: &["accept", "finish"],
    },
    HelpExtra {
        id: "search.escape",
        modes: &[Mode::Search],
        group: HelpExtraGroup::Mode(Mode::Search),
        key: "Esc",
        slug: "",
        description: "Leave search without clearing the query",
        aliases: &["close", "cancel"],
    },
    HelpExtra {
        id: "mouse.wheel",
        modes: &[Mode::Sidebar, Mode::List, Mode::Preview, Mode::Fragment],
        group: HelpExtraGroup::Mouse,
        key: "wheel",
        slug: "",
        description: "Scroll the hovered pane",
        aliases: &["mouse", "scroll"],
    },
    HelpExtra {
        id: "mouse.click",
        modes: &[Mode::Sidebar, Mode::List, Mode::Preview, Mode::Fragment],
        group: HelpExtraGroup::Mouse,
        key: "click",
        slug: "",
        description: "Select an item or fragment",
        aliases: &["mouse", "select"],
    },
    HelpExtra {
        id: "mouse.double-click",
        modes: &[Mode::List],
        group: HelpExtraGroup::Mouse,
        key: "double-click",
        slug: "",
        description: "Drill into the snippet preview",
        aliases: &["mouse", "open", "preview"],
    },
    HelpExtra {
        id: "mouse.drag",
        modes: &[Mode::Preview, Mode::Fragment],
        group: HelpExtraGroup::Mouse,
        key: "drag",
        slug: "",
        description: "Select text in the preview",
        aliases: &["mouse", "selection"],
    },
    HelpExtra {
        id: "mouse.copy-selection",
        modes: &[Mode::Preview, Mode::Fragment],
        group: HelpExtraGroup::Mouse,
        key: "mouse up",
        slug: "",
        description: "Copy the selected preview text",
        aliases: &["mouse", "clipboard", "selection"],
    },
    HelpExtra {
        id: "help.wheel",
        modes: &[Mode::Help],
        group: HelpExtraGroup::HelpControls,
        key: "wheel",
        slug: "",
        description: "Move the help selection",
        aliases: &["mouse", "scroll", "navigate"],
    },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn help_extra_ids_are_unique() {
        let mut ids = HashSet::new();
        for extra in EXTRAS {
            assert!(!extra.id.is_empty());
            assert!(ids.insert(extra.id), "duplicate extra id: {}", extra.id);
        }
    }
}
