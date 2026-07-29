use super::ModalAction;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// A picker row. `label` is what the user reads and filters on; `value` is what the
/// action receives. They differ for the library root, shown as `Uncategorized` but
/// submitted as an empty folder path — which also keeps a real folder of that name
/// from colliding with the root entry.
#[derive(Clone, Debug)]
pub struct PickerItem {
    pub label: String,
    pub value: String,
    pub keywords: Vec<String>,
    fallback_keywords: Vec<String>,
    custom: bool,
}

impl PickerItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            keywords: Vec::new(),
            fallback_keywords: Vec::new(),
            custom: false,
        }
    }

    pub fn with_keywords(
        label: impl Into<String>,
        value: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
            fallback_keywords: Vec::new(),
            custom: false,
        }
    }

    pub fn with_keywords_and_fallbacks(
        label: impl Into<String>,
        value: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
        fallback_keywords: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
            fallback_keywords: fallback_keywords.into_iter().map(Into::into).collect(),
            custom: false,
        }
    }

    /// A row whose label and value are the same, such as a folder path.
    pub fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
            keywords: Vec::new(),
            fallback_keywords: Vec::new(),
            custom: false,
        }
    }

    fn custom(value: &str) -> Self {
        Self {
            label: format!("use “{value}”"),
            value: value.to_owned(),
            keywords: Vec::new(),
            fallback_keywords: Vec::new(),
            custom: true,
        }
    }

    fn match_score(&self, query: &str, pattern: &Pattern, matcher: &mut Matcher) -> Option<u32> {
        [self.label.as_str(), self.value.as_str()]
            .into_iter()
            .chain(self.keywords.iter().map(String::as_str))
            .chain(self.fallback_keywords.iter().map(String::as_str))
            .filter_map(|candidate| {
                let mut buffer = Vec::new();
                pattern.score(Utf32Str::new(candidate, &mut buffer), matcher)
            })
            .max()
            .map(|score| score.saturating_add(self.tier_bonus(query)))
    }

    fn tier_bonus(&self, query: &str) -> u32 {
        const EXACT: u32 = 1 << 24;
        const EXACT_FALLBACK: u32 = 1 << 22;
        const PREFIX: u32 = 1 << 20;

        if self.has_exact_match(query) {
            EXACT
        } else if self
            .fallback_keywords
            .iter()
            .any(|keyword| keyword.eq_ignore_ascii_case(query))
        {
            EXACT_FALLBACK
        } else if [self.label.as_str(), self.value.as_str()]
            .into_iter()
            .chain(self.keywords.iter().map(String::as_str))
            .any(|candidate| starts_with_ignore_ascii_case(candidate, query))
        {
            PREFIX
        } else {
            0
        }
    }

    fn has_exact_match(&self, query: &str) -> bool {
        self.label.eq_ignore_ascii_case(query)
            || self.value.eq_ignore_ascii_case(query)
            || self
                .keywords
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(query))
    }
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

#[derive(Clone, Debug)]
pub struct PickerModal {
    pub label: String,
    items: Vec<PickerItem>,
    filter: String,
    pub selected: usize,
    pub action: ModalAction,
    pub error: Option<String>,
    pub allow_custom: bool,
    pub current_value: Option<String>,
    filtered: Vec<PickerItem>,
}

impl PickerModal {
    pub fn new(label: impl Into<String>, items: Vec<PickerItem>, action: ModalAction) -> Self {
        let filtered = items.clone();
        Self {
            label: label.into(),
            items,
            filter: String::new(),
            selected: 0,
            action,
            error: None,
            allow_custom: false,
            current_value: None,
            filtered,
        }
    }

    pub fn allow_custom(mut self) -> Self {
        self.allow_custom = true;
        self
    }

    pub fn with_current_value(mut self, value: impl Into<String>) -> Self {
        self.current_value = Some(value.into());
        self
    }

    pub fn select_value(&mut self, value: &str) {
        self.selected = self
            .items
            .iter()
            .position(|item| item.value.eq_ignore_ascii_case(value))
            .unwrap_or(0);
    }

    pub fn items(&self) -> &[PickerItem] {
        &self.items
    }

    pub fn replace_items(&mut self, items: Vec<PickerItem>) {
        self.items = items;
        self.rebuild_filtered();
        self.clamp();
    }

    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
        self.rebuild_filtered();
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn push_filter(&mut self, value: char) {
        self.filter.push(value);
        self.rebuild_filtered();
    }

    pub fn pop_filter(&mut self) {
        self.filter.pop();
        self.rebuild_filtered();
    }

    pub fn filtered(&self) -> &[PickerItem] {
        &self.filtered
    }

    fn rebuild_filtered(&mut self) {
        let query = self.filter.trim();
        if query.is_empty() {
            self.filtered.clone_from(&self.items);
            return;
        }
        let exact_match = self.items.iter().any(|item| item.has_exact_match(query));
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut matches = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                item.match_score(query, &pattern, &mut matcher)
                    .map(|score| (score, index, item.clone()))
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        self.filtered = matches
            .into_iter()
            .map(|(_, _, item)| item)
            .collect::<Vec<_>>();
        if self.allow_custom && !exact_match {
            self.filtered.push(PickerItem::custom(query));
        }
    }

    pub fn selected_value(&self) -> Option<String> {
        self.filtered()
            .get(self.selected)
            .map(|item| item.value.clone())
    }

    pub fn clamp(&mut self) {
        let len = self.filtered().len();
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    pub fn title(&self) -> String {
        let current = self
            .current_value
            .as_deref()
            .map_or_else(String::new, |value| format!(" ({value})"));
        let matches = self.filtered().iter().filter(|item| !item.custom).count();
        let direct_use = if self.allow_custom && !self.filter.trim().is_empty() && matches == 0 {
            " · ⏎ direct use"
        } else {
            ""
        };
        format!("{}{current} · {matches} matches{direct_use}", self.label)
    }
}
