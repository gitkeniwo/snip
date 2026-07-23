use ratatui::text::Text;

use crate::domain::Snippet;
use crate::error::Result;

use super::highlight::Highlighter;
use super::theme::TuiTheme;

#[derive(Clone, Debug, Default)]
pub struct PreviewDocument {
    pub note: Option<Text<'static>>,
    pub fragment: Text<'static>,
    pub readme: Option<Text<'static>>,
}

#[derive(Default)]
pub struct PreviewCache {
    key: Option<(String, usize)>,
    document: Option<PreviewDocument>,
}

impl PreviewCache {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.document = None;
    }

    pub fn get(
        &mut self,
        snippet: &Snippet,
        fragment_index: usize,
        highlighter: &Highlighter,
        theme: TuiTheme,
    ) -> Result<PreviewDocument> {
        let key = (snippet.fingerprint.0.clone(), fragment_index);
        if self.key.as_ref() == Some(&key) {
            return Ok(self.document.clone().unwrap_or_default());
        }
        let document = build(snippet, fragment_index, highlighter, theme)?;
        self.key = Some(key);
        self.document = Some(document.clone());
        Ok(document)
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
