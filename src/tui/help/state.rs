use std::ops::Range;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use super::{HelpRow, HelpScope, HelpSort, VisibleHelpRow, project, sort_rows};
use crate::keys::{Keymap, Mode};
use crate::tui::modal::TextInput;

#[derive(Clone, Debug, Default)]
pub struct HelpMatch {
    pub group_indices: Vec<u32>,
    pub key_indices: Vec<u32>,
    pub slug_indices: Vec<u32>,
    pub description_indices: Vec<u32>,
    pub hidden_reason: Option<HiddenMatch>,
}

#[derive(Clone, Debug)]
pub struct HiddenMatch {
    pub field: &'static str,
    pub value: String,
}

pub struct HelpState {
    pub scope: HelpScope,
    pub scope_stack: Vec<Mode>,
    pub sort: HelpSort,
    pub filter: TextInput,
    pub filtering: bool,
    pub selected: usize,
    pub scroll: usize,
    pub viewport_lines: usize,
    pub(crate) row_line_ranges: Vec<Range<usize>>,
    matcher: Matcher,
    rows: Vec<HelpRow>,
    visible: Vec<VisibleHelpRow>,
}

impl Default for HelpState {
    fn default() -> Self {
        Self {
            scope: HelpScope::Context,
            scope_stack: Vec::new(),
            sort: HelpSort::Key,
            filter: TextInput::default(),
            filtering: false,
            selected: 0,
            scroll: 0,
            viewport_lines: 1,
            row_line_ranges: Vec::new(),
            matcher: Matcher::new(Config::DEFAULT),
            rows: Vec::new(),
            visible: Vec::new(),
        }
    }
}

impl HelpState {
    pub fn open(&mut self, scope_stack: Vec<Mode>, keymap: &Keymap, gui_editor: Option<&str>) {
        self.scope = HelpScope::Context;
        self.scope_stack = scope_stack;
        self.filter = TextInput::default();
        self.filtering = false;
        self.selected = 0;
        self.scroll = 0;
        self.rebuild(keymap, gui_editor);
    }

    pub fn close(&mut self) {
        self.filtering = false;
        self.filter = TextInput::default();
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn start_filtering(&mut self) {
        self.filtering = true;
    }

    pub fn leave_filtering(&mut self, clear: bool) {
        self.filtering = false;
        if clear {
            self.filter = TextInput::default();
            self.refresh_filter();
        }
    }

    pub fn toggle_scope(&mut self, keymap: &Keymap, gui_editor: Option<&str>) {
        self.scope = match self.scope {
            HelpScope::Context => HelpScope::All,
            HelpScope::All => HelpScope::Context,
        };
        self.rebuild(keymap, gui_editor);
    }

    pub fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            HelpSort::Key => HelpSort::Action,
            HelpSort::Action => HelpSort::Key,
        };
        self.rebuild_visible_preserving_selection();
    }

    pub fn refresh_filter(&mut self) {
        self.rebuild_visible_preserving_selection();
    }

    pub fn visible_rows(&self) -> &[VisibleHelpRow] {
        &self.visible
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected =
            (self.selected as isize + delta).clamp(0, self.visible.len() as isize - 1) as usize;
        self.ensure_selected_visible();
    }

    pub fn move_half_page(&mut self, direction: isize) {
        if self.visible.is_empty() {
            return;
        }
        let current_line = self
            .row_line_ranges
            .get(self.selected)
            .map_or(self.selected, |range| range.start);
        let distance = self.viewport_lines.max(2) / 2;
        let target = if direction < 0 {
            current_line.saturating_sub(distance)
        } else {
            current_line.saturating_add(distance)
        };
        self.selected = self
            .row_line_ranges
            .iter()
            .enumerate()
            .min_by_key(|(_, range)| range.start.abs_diff(target))
            .map_or(self.selected, |(index, _)| index);
        self.ensure_selected_visible();
    }

    pub(crate) fn update_layout(
        &mut self,
        viewport_lines: usize,
        row_line_ranges: Vec<Range<usize>>,
    ) {
        self.viewport_lines = viewport_lines.max(1);
        self.row_line_ranges = row_line_ranges;
        let content_lines = self.row_line_ranges.last().map_or(0, |range| range.end);
        self.scroll = self
            .scroll
            .min(content_lines.saturating_sub(self.viewport_lines));
        self.ensure_selected_visible();
    }

    pub fn count(&self) -> usize {
        self.visible.len()
    }

    pub fn total_count(&self) -> usize {
        self.rows.len()
    }

    fn rebuild(&mut self, keymap: &Keymap, gui_editor: Option<&str>) {
        self.rows = project(self.scope, &self.scope_stack, keymap, gui_editor);
        let group_order = self.group_order();
        sort_rows(&mut self.rows, self.sort, &group_order);
        self.rebuild_visible_preserving_selection();
    }

    fn rebuild_visible_preserving_selection(&mut self) {
        let previous_index = self.selected;
        let selected_id = self.visible.get(self.selected).map(|row| row.row.id);
        let group_order = self.group_order();
        sort_rows(&mut self.rows, self.sort, &group_order);
        let query = self.filter.value.trim();
        self.visible = if query.is_empty() {
            self.rows
                .iter()
                .cloned()
                .map(|row| VisibleHelpRow {
                    row,
                    matched: HelpMatch::default(),
                })
                .collect()
        } else {
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let matcher = &mut self.matcher;
            self.rows
                .iter()
                .filter_map(|row| {
                    match_row(row, &pattern, matcher).map(|matched| VisibleHelpRow {
                        row: row.clone(),
                        matched,
                    })
                })
                .collect()
        };
        self.selected = selected_id
            .and_then(|id| self.visible.iter().position(|row| row.row.id == id))
            .unwrap_or_else(|| previous_index.min(self.visible.len().saturating_sub(1)));
        self.scroll = 0;
        self.ensure_selected_visible();
    }

    fn group_order(&self) -> Vec<Mode> {
        match self.scope {
            HelpScope::Context => self.scope_stack.clone(),
            HelpScope::All => Mode::ALL.to_vec(),
        }
    }

    fn ensure_selected_visible(&mut self) {
        if self.visible.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = self.selected.min(self.visible.len() - 1);
        let Some(range) = self.row_line_ranges.get(self.selected).cloned() else {
            return;
        };
        if range.start < self.scroll {
            self.scroll = range.start;
        } else if range.end > self.scroll.saturating_add(self.viewport_lines) {
            self.scroll = range.end.saturating_sub(self.viewport_lines);
        }
    }
}

fn match_row(row: &HelpRow, pattern: &Pattern, matcher: &mut Matcher) -> Option<HelpMatch> {
    let group_indices = field_indices(pattern, matcher, row.display_group.label());
    let key_indices = field_indices(pattern, matcher, &row.key.display());
    let slug_indices = field_indices(pattern, matcher, row.slug);
    let description_indices = field_indices(pattern, matcher, row.description);
    let displayed = group_indices.is_some()
        || key_indices.is_some()
        || slug_indices.is_some()
        || description_indices.is_some();

    let hidden_reason = if displayed {
        None
    } else {
        std::iter::once(("title", row.title.as_ref()))
            .chain(std::iter::once(("category", row.category)))
            .chain(row.keywords.iter().copied().map(|value| ("keyword", value)))
            .chain(row.aliases.iter().copied().map(|value| ("alias", value)))
            .find(|(_, value)| field_indices(pattern, matcher, value).is_some())
            .map(|(field, value)| HiddenMatch {
                field,
                value: value.to_owned(),
            })
    };
    if !displayed && hidden_reason.is_none() {
        return None;
    }
    Some(HelpMatch {
        group_indices: group_indices.unwrap_or_default(),
        key_indices: key_indices.unwrap_or_default(),
        slug_indices: slug_indices.unwrap_or_default(),
        description_indices: description_indices.unwrap_or_default(),
        hidden_reason,
    })
}

fn field_indices(pattern: &Pattern, matcher: &mut Matcher, value: &str) -> Option<Vec<u32>> {
    if value.is_empty() {
        return None;
    }
    let mut buffer = Vec::new();
    let mut indices = Vec::new();
    pattern
        .indices(Utf32Str::new(value, &mut buffer), matcher, &mut indices)
        .map(|_| indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matches_hidden_fields_and_preserves_selection() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::List, Mode::Global], &keymap, None);
        state.filter.value = "force".to_owned();
        state.refresh_filter();
        assert!(
            state
                .visible
                .iter()
                .any(|row| row.row.slug == "view.cycle-appearance")
        );
        assert!(
            state
                .visible
                .iter()
                .any(|row| row.matched.hidden_reason.is_some())
        );
    }

    #[test]
    fn scope_change_preserves_stable_global_identity() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::Help], &keymap, None);
        state.selected = state
            .visible
            .iter()
            .position(|row| row.row.slug == "git.toggle-console")
            .unwrap();
        state.toggle_scope(&keymap, None);
        assert_eq!(state.visible[state.selected].row.slug, "git.toggle-console");
    }

    #[test]
    fn growing_viewport_reclamps_scroll_without_bottom_blank_space() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::List, Mode::Global], &keymap, None);
        let content_lines = state.count();
        let ranges = (0..content_lines)
            .map(|line| line..line + 1)
            .collect::<Vec<_>>();
        state.selected = state.count() - 1;

        state.update_layout(5, ranges.clone());
        assert_eq!(state.scroll, content_lines - 5);

        state.update_layout(20, ranges);
        assert_eq!(state.scroll, content_lines - 20);
        assert_eq!(state.scroll + state.viewport_lines, content_lines);
    }
}
