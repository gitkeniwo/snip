use regex::{Regex, RegexBuilder};

use crate::domain::{CatalogSnapshot, FolderFilter, SearchField, SearchResult, Snippet};
use crate::error::{Result, SnipError};

pub trait SearchIndex {
    fn search(&self, query: &SearchQuery<'_>) -> Vec<SearchResult>;
}

/// How a pattern is compared against text. Both forms are case-insensitive, which
/// is what people expect when searching their own notes; a regex can opt out
/// inline with `(?-i)` rather than needing another flag.
#[derive(Clone, Debug)]
enum Matcher {
    Substring(String),
    Regex(Box<Regex>),
}

impl Matcher {
    fn new(pattern: &str, regex: bool) -> Result<Self> {
        if !regex {
            return Ok(Self::Substring(pattern.to_lowercase()));
        }
        RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .map(|regex| Self::Regex(Box::new(regex)))
            .map_err(|error| SnipError::usage(format!("invalid regular expression: {error}")))
    }

    fn is_match(&self, haystack: &str) -> bool {
        match self {
            Self::Substring(needle) => haystack.to_lowercase().contains(needle.as_str()),
            Self::Regex(regex) => regex.is_match(haystack),
        }
    }

    /// Whether the pattern covers the whole value, which is what earns a title
    /// the top score.
    fn is_exact(&self, haystack: &str) -> bool {
        match self {
            Self::Substring(needle) => haystack.to_lowercase() == *needle,
            Self::Regex(regex) => regex
                .find(haystack)
                .is_some_and(|found| found.start() == 0 && found.end() == haystack.len()),
        }
    }
}

/// A complete search request. Built once and shared by `snip search` and the TUI
/// so both surfaces match, score, and filter identically.
#[derive(Clone, Debug)]
pub struct SearchQuery<'a> {
    matcher: Matcher,
    folder: Option<FolderFilter<'a>>,
    tag: Option<&'a str>,
    fields: Vec<SearchField>,
    context_lines: usize,
    limit: Option<usize>,
}

impl<'a> SearchQuery<'a> {
    /// Fails only when `regex` is set and the pattern does not compile.
    pub fn new(pattern: &str, regex: bool) -> Result<Self> {
        Ok(Self {
            matcher: Matcher::new(pattern, regex)?,
            folder: None,
            tag: None,
            fields: Vec::new(),
            context_lines: 0,
            limit: None,
        })
    }

    pub fn folder(mut self, folder: Option<FolderFilter<'a>>) -> Self {
        self.folder = folder;
        self
    }

    pub fn tag(mut self, tag: Option<&'a str>) -> Self {
        self.tag = tag;
        self
    }

    /// An empty selection searches everything.
    pub fn fields(mut self, fields: &[SearchField]) -> Self {
        self.fields = fields.to_vec();
        self
    }

    pub fn context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }

    pub fn limit(mut self, limit: Option<usize>) -> Self {
        self.limit = limit;
        self
    }

    fn wants(&self, field: SearchField) -> bool {
        self.fields.is_empty() || self.fields.contains(&field)
    }
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
    fn search(&self, query: &SearchQuery<'_>) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for snippet in &self.catalog.snippets {
            if query
                .folder
                .is_some_and(|folder| !folder.matches(&snippet.folder))
            {
                continue;
            }
            if query.tag.is_some_and(|tag| {
                !snippet
                    .tags
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(tag))
            }) {
                continue;
            }

            if query.wants(SearchField::Title) && query.matcher.is_match(&snippet.title) {
                results.push(base_result(
                    snippet,
                    SearchField::Title,
                    None,
                    None,
                    snippet.title.clone(),
                    if query.matcher.is_exact(&snippet.title) {
                        100
                    } else {
                        80
                    },
                ));
            }
            if query.wants(SearchField::Tag)
                && let Some(tag) = snippet
                    .tags
                    .iter()
                    .find(|candidate| query.matcher.is_match(candidate))
            {
                results.push(base_result(
                    snippet,
                    SearchField::Tag,
                    None,
                    None,
                    format!("tag: {tag}"),
                    65,
                ));
            }
            if query.wants(SearchField::Readme)
                && let Some(readme) = &snippet.readme
            {
                push_line_matches(
                    &mut results,
                    snippet,
                    SearchField::Readme,
                    None,
                    readme,
                    query,
                    50,
                );
            }
            for fragment in &snippet.loaded_fragments {
                let source = Some((fragment.id, fragment.title.as_str()));
                if query.wants(SearchField::Content) {
                    push_line_matches(
                        &mut results,
                        snippet,
                        SearchField::Content,
                        source,
                        &fragment.content,
                        query,
                        40,
                    );
                }
                if query.wants(SearchField::Note)
                    && let Some(note) = &fragment.note_content
                {
                    push_line_matches(
                        &mut results,
                        snippet,
                        SearchField::Note,
                        source,
                        note,
                        query,
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
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        results
    }
}

fn base_result(
    snippet: &Snippet,
    field: SearchField,
    fragment: Option<(uuid::Uuid, &str)>,
    line: Option<usize>,
    excerpt: String,
    score: u32,
) -> SearchResult {
    SearchResult {
        snippet_id: snippet.id,
        title: snippet.title.clone(),
        folder: snippet.folder.clone(),
        fingerprint: snippet.fingerprint.clone(),
        field,
        fragment_id: fragment.map(|value| value.0),
        fragment_title: fragment.map(|value| value.1.to_owned()),
        line,
        excerpt,
        context_before: Vec::new(),
        context_after: Vec::new(),
        score,
    }
}

fn push_line_matches(
    results: &mut Vec<SearchResult>,
    snippet: &Snippet,
    field: SearchField,
    fragment: Option<(uuid::Uuid, &str)>,
    text: &str,
    query: &SearchQuery<'_>,
    score: u32,
) {
    let lines = text.lines().collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        if !query.matcher.is_match(line) {
            continue;
        }
        let mut result = base_result(
            snippet,
            field,
            fragment,
            Some(index + 1),
            line.trim().to_owned(),
            score,
        );
        if query.context_lines > 0 {
            let start = index.saturating_sub(query.context_lines);
            let end = (index + query.context_lines + 1).min(lines.len());
            result.context_before = lines[start..index]
                .iter()
                .map(|line| (*line).to_owned())
                .collect();
            result.context_after = lines[index + 1..end]
                .iter()
                .map(|line| (*line).to_owned())
                .collect();
        }
        results.push(result);
    }
}
