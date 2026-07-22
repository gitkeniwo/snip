use crate::domain::Snippet;

/// A portable, fixed-width alternative to Nerd Font private-use glyphs.
pub fn snippet_badge(snippet: &Snippet) -> &'static str {
    let Some(first) = snippet.loaded_fragments.first() else {
        return "--";
    };
    if snippet
        .loaded_fragments
        .iter()
        .skip(1)
        .any(|fragment| !fragment.language.eq_ignore_ascii_case(&first.language))
    {
        return "++";
    }
    language_badge(&first.language)
}

pub fn language_badge(language: &str) -> &'static str {
    match language.trim().to_ascii_lowercase().as_str() {
        "rust" => "rs",
        "python" => "py",
        "bash" | "shell" | "sh" | "fish" | "zsh" => "sh",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "go" | "golang" => "go",
        "sql" => "db",
        "markdown" | "md" => "md",
        "html" | "xml" => "<>",
        "css" | "scss" | "sass" => "# ",
        "json" => "{}",
        "yaml" | "yml" => "ym",
        "toml" => "tm",
        "dockerfile" | "docker" => "dk",
        "makefile" | "make" => "mk",
        "swift" => "sw",
        "kotlin" => "kt",
        "java" => "jv",
        "c" => "c ",
        "cpp" | "c++" => "c+",
        "text" | "plain" => "--",
        _ => "··",
    }
}

#[cfg(test)]
mod tests {
    use super::language_badge;

    #[test]
    fn common_languages_have_font_independent_badges() {
        assert_eq!(language_badge("Rust"), "rs");
        assert_eq!(language_badge("fish"), "sh");
        assert_eq!(language_badge("JSON"), "{}");
        assert_eq!(language_badge("unknown-language"), "··");
    }
}
