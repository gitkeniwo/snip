use crate::domain::{CatalogSnapshot, SearchResult};

pub trait SearchIndex {
    fn search(&self, query: &str, folder: Option<&str>, tag: Option<&str>) -> Vec<SearchResult>;
}

#[derive(Clone, Debug)]
pub struct MemoryIndex {
    catalog: CatalogSnapshot,
}

impl MemoryIndex {
    pub fn new(catalog: CatalogSnapshot) -> Self {
        Self { catalog }
    }

    pub fn catalog(&self) -> &CatalogSnapshot {
        &self.catalog
    }
}

impl SearchIndex for MemoryIndex {
    fn search(&self, query: &str, folder: Option<&str>, tag: Option<&str>) -> Vec<SearchResult> {
        let needle = query.to_lowercase();
        let folder_filter = folder.map(str::to_lowercase);
        let tag_filter = tag.map(str::to_lowercase);
        let mut results = Vec::new();
        for snippet in &self.catalog.snippets {
            if folder_filter
                .as_ref()
                .is_some_and(|folder| snippet.folder.to_lowercase() != *folder)
            {
                continue;
            }
            if tag_filter.as_ref().is_some_and(|tag| {
                !snippet
                    .tags
                    .iter()
                    .any(|candidate| candidate.to_lowercase() == *tag)
            }) {
                continue;
            }

            let title_lower = snippet.title.to_lowercase();
            if title_lower.contains(&needle) {
                results.push(SearchResult {
                    snippet_id: snippet.id,
                    title: snippet.title.clone(),
                    folder: snippet.folder.clone(),
                    fragment_id: None,
                    fragment_title: None,
                    line: None,
                    excerpt: snippet.title.clone(),
                    score: if title_lower == needle { 100 } else { 80 },
                });
            }
            if let Some(tag_name) = snippet
                .tags
                .iter()
                .find(|candidate| candidate.to_lowercase().contains(&needle))
            {
                results.push(SearchResult {
                    snippet_id: snippet.id,
                    title: snippet.title.clone(),
                    folder: snippet.folder.clone(),
                    fragment_id: None,
                    fragment_title: None,
                    line: None,
                    excerpt: format!("tag: {tag_name}"),
                    score: 65,
                });
            }
            if let Some(readme) = &snippet.readme {
                push_line_matches(&mut results, snippet, None, readme, &needle, 50);
            }
            for fragment in &snippet.loaded_fragments {
                push_line_matches(
                    &mut results,
                    snippet,
                    Some((fragment.id, fragment.title.as_str())),
                    &fragment.content,
                    &needle,
                    40,
                );
                if let Some(note) = &fragment.note_content {
                    push_line_matches(
                        &mut results,
                        snippet,
                        Some((fragment.id, fragment.title.as_str())),
                        note,
                        &needle,
                        45,
                    );
                }
            }
        }
        results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
                .then_with(|| left.line.cmp(&right.line))
        });
        results
    }
}

fn push_line_matches(
    results: &mut Vec<SearchResult>,
    snippet: &crate::domain::Snippet,
    fragment: Option<(uuid::Uuid, &str)>,
    text: &str,
    needle: &str,
    score: u32,
) {
    for (index, line) in text.lines().enumerate() {
        if line.to_lowercase().contains(needle) {
            results.push(SearchResult {
                snippet_id: snippet.id,
                title: snippet.title.clone(),
                folder: snippet.folder.clone(),
                fragment_id: fragment.map(|value| value.0),
                fragment_title: fragment.map(|value| value.1.to_owned()),
                line: Some(index + 1),
                excerpt: line.trim().to_owned(),
                score,
            });
        }
    }
}
