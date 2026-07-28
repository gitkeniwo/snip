use crate::domain::Snippet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconMode {
    #[default]
    Ascii,
    Nerd,
}

/// A portable, fixed-width alternative to Nerd Font private-use glyphs.
pub fn snippet_badge(snippet: &Snippet) -> &'static str {
    let Some(first) = snippet.loaded_fragments.first() else {
        return "txt";
    };
    if snippet
        .loaded_fragments
        .iter()
        .skip(1)
        .any(|fragment| !fragment.language.eq_ignore_ascii_case(&first.language))
    {
        return "mix";
    }
    language_badge(&first.language)
}

pub fn language_badge(language: &str) -> &'static str {
    crate::language::info(language).map_or("?", |info| info.badge)
}

pub fn language_name(language: &str) -> String {
    crate::language::info(language).map_or_else(
        || language.trim().to_owned(),
        |info| info.canonical_name.to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::{language_badge, language_name};

    #[test]
    fn common_languages_have_font_independent_badges() {
        assert_eq!(language_badge("Rust"), "rs");
        assert_eq!(language_badge("fish"), "fsh");
        assert_eq!(language_badge("JSON"), "jsn");
        assert_eq!(language_badge("unknown-language"), "?");
    }

    #[test]
    fn language_names_are_human_readable_and_preserve_unknown_values() {
        assert_eq!(language_name("js"), "JavaScript");
        assert_eq!(language_name("cpp"), "C++");
        assert_eq!(language_name("fish"), "Fish");
        assert_eq!(language_name("custom-lang"), "custom-lang");
    }
}
