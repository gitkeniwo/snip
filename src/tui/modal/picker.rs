use super::ModalAction;

/// A picker row. `label` is what the user reads and filters on; `value` is what the
/// action receives. They differ for the library root, shown as `Uncategorized` but
/// submitted as an empty folder path — which also keeps a real folder of that name
/// from colliding with the root entry.
#[derive(Clone, Debug)]
pub struct PickerItem {
    pub label: String,
    pub value: String,
    pub keywords: Vec<String>,
    custom: bool,
}

impl PickerItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            keywords: Vec::new(),
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
            custom: false,
        }
    }

    fn custom(value: &str) -> Self {
        Self {
            label: format!("use “{value}”"),
            value: value.to_owned(),
            keywords: Vec::new(),
            custom: true,
        }
    }

    fn match_rank(&self, query: &str) -> Option<u8> {
        let label = self.label.to_lowercase();
        let value = self.value.to_lowercase();
        let keywords = self
            .keywords
            .iter()
            .map(|keyword| keyword.to_lowercase())
            .collect::<Vec<_>>();
        if label == query || value == query || keywords.iter().any(|keyword| keyword == query) {
            Some(0)
        } else if label.starts_with(query) || value.starts_with(query) {
            Some(1)
        } else if keywords.iter().any(|keyword| keyword.starts_with(query)) {
            Some(2)
        } else if label.contains(query) || value.contains(query) {
            Some(3)
        } else if keywords.iter().any(|keyword| keyword.contains(query)) {
            Some(4)
        } else {
            None
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

#[derive(Clone, Debug)]
pub struct PickerModal {
    pub label: String,
    pub items: Vec<PickerItem>,
    pub filter: String,
    pub selected: usize,
    pub action: ModalAction,
    pub error: Option<String>,
    pub allow_custom: bool,
    pub current_value: Option<String>,
}

impl PickerModal {
    pub fn new(label: impl Into<String>, items: Vec<PickerItem>, action: ModalAction) -> Self {
        Self {
            label: label.into(),
            items,
            filter: String::new(),
            selected: 0,
            action,
            error: None,
            allow_custom: false,
            current_value: None,
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

    pub fn filtered(&self) -> Vec<PickerItem> {
        let query = self.filter.trim().to_lowercase();
        if query.is_empty() {
            return self.items.clone();
        }
        let exact_match = self.items.iter().any(|item| item.has_exact_match(&query));
        let mut matches = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                item.match_rank(&query)
                    .map(|rank| (rank, index, item.clone()))
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(rank, index, _)| (*rank, *index));
        let mut items = matches
            .into_iter()
            .map(|(_, _, item)| item)
            .collect::<Vec<_>>();
        if self.allow_custom && !exact_match {
            items.push(PickerItem::custom(self.filter.trim()));
        }
        items
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
        let matches = self
            .filtered()
            .into_iter()
            .filter(|item| !item.custom)
            .count();
        let direct_use = if self.allow_custom && !self.filter.trim().is_empty() && matches == 0 {
            " · ⏎ direct use"
        } else {
            ""
        };
        format!("{}{current} · {matches} matches{direct_use}", self.label)
    }
}
