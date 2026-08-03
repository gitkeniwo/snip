use ratatui::text::Text;

use crate::domain::Snippet;
use crate::error::Result;

use super::super::highlight::Highlighter;
use super::super::theme::TuiTheme;
use super::layout::{WrappedPreview, compose_preview, wrap_preview};

#[derive(Clone, Debug, Default)]
pub struct PreviewDocument {
    pub note: Option<Text<'static>>,
    pub fragment: Text<'static>,
    pub readme: Option<Text<'static>>,
}

#[derive(Default)]
pub struct PreviewCache {
    key: Option<(String, usize, u16, bool)>,
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
        fragment_index: usize,
        content_width: u16,
        show_line_numbers: bool,
        highlighter: &Highlighter,
        theme: TuiTheme,
    ) -> Result<(&WrappedPreview, bool)> {
        let hit = self.key.as_ref().is_some_and(|key| {
            key.0 == snippet.fingerprint.0
                && key.1 == fragment_index
                && key.2 == content_width
                && key.3 == show_line_numbers
        });
        if hit {
            return Ok((
                self.preview.as_ref().expect("cache key without preview"),
                false,
            ));
        }
        let document = build(snippet, fragment_index, highlighter, theme)?;
        let lines = compose_preview(document, show_line_numbers, theme, content_width);
        let preview = wrap_preview(lines, content_width, show_line_numbers);
        self.key = Some((
            snippet.fingerprint.0.clone(),
            fragment_index,
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
    fragment_index: usize,
    highlighter: &Highlighter,
    theme: TuiTheme,
) -> Result<PreviewDocument> {
    let Some(fragment) = snippet.loaded_fragments.get(fragment_index) else {
        return Ok(PreviewDocument::default());
    };
    Ok(PreviewDocument {
        note: fragment
            .note_content
            .as_deref()
            .map(|note| highlighter.markdown(note, theme)),
        fragment: highlighter.fragment(fragment)?,
        readme: snippet
            .readme
            .as_deref()
            .map(|readme| highlighter.markdown(readme, theme)),
    })
}
