use crate::domain::Snippet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconMode {
    #[default]
    Ascii,
    Nerd,
}

impl IconMode {
    /// Nerd Font glyphs are intentionally deferred; v2 remains portable.
    pub fn effective(self) -> Self {
        Self::Ascii
    }
}

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

pub fn language_name(language: &str) -> String {
    let normalized = language.trim().to_ascii_lowercase();
    let name = match normalized.as_str() {
        "rust" => "Rust",
        "python" => "Python",
        "bash" => "Bash",
        "shell" | "sh" => "Shell",
        "fish" => "Fish",
        "zsh" => "Zsh",
        "javascript" | "js" => "JavaScript",
        "typescript" | "ts" => "TypeScript",
        "go" | "golang" => "Go",
        "sql" => "SQL",
        "markdown" | "md" => "Markdown",
        "html" => "HTML",
        "xml" => "XML",
        "css" => "CSS",
        "scss" => "SCSS",
        "sass" => "Sass",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "dockerfile" | "docker" => "Dockerfile",
        "makefile" | "make" => "Makefile",
        "swift" => "Swift",
        "kotlin" => "Kotlin",
        "java" => "Java",
        "c" => "C",
        "cpp" | "c++" => "C++",
        "text" | "plain" | "" => "Plain Text",
        _ => return language.trim().to_owned(),
    };
    name.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{language_badge, language_name};

    #[test]
    fn common_languages_have_font_independent_badges() {
        assert_eq!(language_badge("Rust"), "rs");
        assert_eq!(language_badge("fish"), "sh");
        assert_eq!(language_badge("JSON"), "{}");
        assert_eq!(language_badge("unknown-language"), "··");
    }

    #[test]
    fn language_names_are_human_readable_and_preserve_unknown_values() {
        assert_eq!(language_name("js"), "JavaScript");
        assert_eq!(language_name("cpp"), "C++");
        assert_eq!(language_name("fish"), "Fish");
        assert_eq!(language_name("custom-lang"), "custom-lang");
    }
}
