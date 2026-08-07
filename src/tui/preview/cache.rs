use ratatui::text::Text;

use crate::domain::Snippet;
use crate::error::Result;

use super::super::highlight::Highlighter;
use super::super::theme::TuiTheme;
use super::layout::{WrappedPreview, compose_preview, wrap_preview};
use super::{PreviewTarget, has_readme};

/// The preview pane's content, in exactly one of its three shapes. Parallel
/// fields would let a fragment body carry a README tail; this enum cannot.
#[derive(Clone, Debug, Default)]
pub enum PreviewDocument {
    Fragment {
        note: Option<Text<'static>>,
        body: Text<'static>,
    },
    Readme(Text<'static>),
    #[default]
    Empty,
}

#[derive(Default)]
pub struct PreviewCache {
    key: Option<(String, PreviewTarget, u16, bool)>,
    preview: Option<WrappedPreview>,
}

impl PreviewCache {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.preview = None;
    }

    /// Theme changes rely on callers explicitly invalidating this cache.
    pub fn get(
        &mut self,
        snippet: &Snippet,
        target: PreviewTarget,
        content_width: u16,
        show_line_numbers: bool,
        highlighter: &Highlighter,
        theme: TuiTheme,
    ) -> Result<(&WrappedPreview, bool)> {
        let hit = self.key.as_ref().is_some_and(|key| {
            key.0 == snippet.fingerprint.0
                && key.1 == target
                && key.2 == content_width
                && key.3 == show_line_numbers
        });
        if hit {
            return Ok((
                self.preview.as_ref().expect("cache key without preview"),
                false,
            ));
        }
        let document = build(snippet, target, highlighter, theme)?;
        let lines = compose_preview(document, show_line_numbers, theme, content_width);
        let preview = wrap_preview(lines, content_width, show_line_numbers);
        self.key = Some((
            snippet.fingerprint.0.clone(),
            target,
            content_width,
            show_line_numbers,
        ));
        self.preview = Some(preview);
        Ok((
            self.preview.as_ref().expect("preview was just cached"),
            true,
        ))
    }
}

fn build(
    snippet: &Snippet,
    target: PreviewTarget,
    highlighter: &Highlighter,
    theme: TuiTheme,
) -> Result<PreviewDocument> {
    let Some(fragment_index) = target.fragment_index() else {
        // Gated on `has_readme`, the same predicate the tree row and the
        // switching order use, so an empty README is absent everywhere.
        return Ok(if has_readme(snippet) {
            PreviewDocument::Readme(
                highlighter.markdown(snippet.readme.as_deref().unwrap_or_default(), theme),
            )
        } else {
            PreviewDocument::Empty
        });
    };
    let Some(fragment) = snippet.loaded_fragments.get(fragment_index) else {
        return Ok(PreviewDocument::Empty);
    };
    // A fragment preview never reads the README: snippet-level prose has its own
    // target and must not trail the fragment body.
    Ok(PreviewDocument::Fragment {
        note: fragment
            .note_content
            .as_deref()
            .map(|note| highlighter.markdown(note, theme)),
        body: highlighter.fragment(fragment)?,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::*;
    use crate::domain::{Fingerprint, Fragment, FragmentManifest, SnippetManifest};

    fn snippet(readme: Option<&str>) -> Snippet {
        let manifest = FragmentManifest {
            id: Uuid::new_v4(),
            title: "body".to_owned(),
            language: "markdown".to_owned(),
            file: "fragments/001-body.md".to_owned(),
            note: None,
            source_language: None,
            extra: toml::Table::new(),
        };
        let fragment = Fragment {
            manifest: manifest.clone(),
            content: "fragment body\n".to_owned(),
            note_content: Some("a note".to_owned()),
            absolute_path: PathBuf::from("fragments/001-body.md"),
        };
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Snippet".to_owned(),
                tags: Vec::new(),
                pinned: false,
                locked: false,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                source: None,
                remotes: Vec::new(),
                fragments: vec![manifest],
                extra: toml::Table::new(),
            },
            readme: readme.map(str::to_owned),
            folder: String::new(),
            package_path: PathBuf::new(),
            modified_at: None,
            fingerprint: Fingerprint("test".to_owned()),
            loaded_fragments: vec![fragment],
        }
    }

    fn highlighter() -> (Highlighter, TuiTheme) {
        let source = crate::theme::load("dark-default").unwrap();
        (Highlighter::new(&source).unwrap(), TuiTheme::from(&source))
    }

    fn plain(text: &Text<'static>) -> String {
        text.lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect()
    }

    #[test]
    fn fragment_document_omits_readme() {
        let (highlighter, theme) = highlighter();
        let snippet = snippet(Some("snippet level prose\n"));
        let document = build(&snippet, PreviewTarget::Fragment(0), &highlighter, theme).unwrap();

        let PreviewDocument::Fragment { note, body } = document else {
            panic!("a fragment target must build a fragment document");
        };
        assert!(!plain(&body).contains("snippet level prose"));
        assert!(!plain(&note.unwrap()).contains("snippet level prose"));
    }

    #[test]
    fn readme_target_builds_a_readme_document() {
        let (highlighter, theme) = highlighter();
        let document = build(
            &snippet(Some("snippet level prose\n")),
            PreviewTarget::Readme,
            &highlighter,
            theme,
        )
        .unwrap();

        let PreviewDocument::Readme(text) = document else {
            panic!("a readme target must build a readme document");
        };
        assert!(plain(&text).contains("snippet level prose"));
    }

    /// An empty README is no README, on the same predicate the tree row and the
    /// switching order use.
    #[test]
    fn a_missing_or_empty_readme_builds_an_empty_document() {
        let (highlighter, theme) = highlighter();
        for readme in [None, Some("")] {
            let document =
                build(&snippet(readme), PreviewTarget::Readme, &highlighter, theme).unwrap();
            assert!(matches!(document, PreviewDocument::Empty), "{readme:?}");
        }
    }

    #[test]
    fn cache_misses_across_targets() {
        let (highlighter, theme) = highlighter();
        let snippet = snippet(Some("snippet level prose\n"));
        let mut cache = PreviewCache::default();
        let rebuild = |cache: &mut PreviewCache, target| {
            cache
                .get(&snippet, target, 40, false, &highlighter, theme)
                .unwrap()
                .1
        };

        assert!(rebuild(&mut cache, PreviewTarget::Fragment(0)));
        assert!(rebuild(&mut cache, PreviewTarget::Readme));
        assert!(rebuild(&mut cache, PreviewTarget::Fragment(0)));
    }
}
