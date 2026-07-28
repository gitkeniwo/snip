#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageInfo {
    pub canonical_name: &'static str,
    pub extension: Option<&'static str>,
    pub badge: &'static str,
}

pub fn info(language: &str) -> Option<LanguageInfo> {
    let info = match language.trim().to_ascii_lowercase().as_str() {
        "bash" => LanguageInfo::new("Bash", Some("sh"), "sh"),
        "shell" | "sh" => LanguageInfo::new("Shell", Some("sh"), "sh"),
        "fish" => LanguageInfo::new("Fish", Some("fish"), "fsh"),
        "zsh" => LanguageInfo::new("Zsh", Some("zsh"), "zsh"),
        "python" => LanguageInfo::new("Python", Some("py"), "py"),
        "rust" => LanguageInfo::new("Rust", Some("rs"), "rs"),
        "javascript" | "js" => LanguageInfo::new("JavaScript", Some("js"), "js"),
        "typescript" | "ts" => LanguageInfo::new("TypeScript", Some("ts"), "ts"),
        "json" => LanguageInfo::new("JSON", Some("json"), "jsn"),
        "yaml" | "yml" => LanguageInfo::new("YAML", Some("yaml"), "yml"),
        "toml" => LanguageInfo::new("TOML", Some("toml"), "tml"),
        "markdown" | "md" => LanguageInfo::new("Markdown", Some("md"), "md"),
        "html" => LanguageInfo::new("HTML", Some("html"), "htm"),
        "xml" => LanguageInfo::new("XML", Some("xml"), "xml"),
        "css" => LanguageInfo::new("CSS", Some("css"), "css"),
        "scss" => LanguageInfo::new("SCSS", Some("scss"), "scs"),
        "sass" => LanguageInfo::new("Sass", Some("sass"), "scs"),
        "sql" => LanguageInfo::new("SQL", Some("sql"), "sql"),
        "go" | "golang" => LanguageInfo::new("Go", Some("go"), "go"),
        "tcl" => LanguageInfo::new("Tcl", Some("tcl"), "tcl"),
        "dockerfile" | "docker" => LanguageInfo::new("Dockerfile", None, "dkr"),
        "makefile" | "make" => LanguageInfo::new("Makefile", None, "mk"),
        "swift" => LanguageInfo::new("Swift", Some("swift"), "swf"),
        "kotlin" => LanguageInfo::new("Kotlin", Some("kt"), "kt"),
        "java" => LanguageInfo::new("Java", Some("java"), "jav"),
        "c" => LanguageInfo::new("C", Some("c"), "c"),
        "cpp" | "c++" => LanguageInfo::new("C++", Some("cpp"), "cpp"),
        "text" | "plain" | "" => LanguageInfo::new("Plain Text", None, "txt"),
        _ => return None,
    };
    Some(info)
}

impl LanguageInfo {
    const fn new(
        canonical_name: &'static str,
        extension: Option<&'static str>,
        badge: &'static str,
    ) -> Self {
        Self {
            canonical_name,
            extension,
            badge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_languages_remain_visually_and_structurally_distinct() {
        assert_eq!(info("sh").unwrap().badge, "sh");
        assert_eq!(info("fish").unwrap().badge, "fsh");
        assert_eq!(info("fish").unwrap().extension, Some("fish"));
        assert_eq!(info("zsh").unwrap().badge, "zsh");
    }

    #[test]
    fn every_known_badge_fits_the_three_cell_column() {
        for language in [
            "bash",
            "fish",
            "zsh",
            "python",
            "rust",
            "javascript",
            "typescript",
            "json",
            "yaml",
            "toml",
            "markdown",
            "html",
            "xml",
            "css",
            "scss",
            "sql",
            "go",
            "tcl",
            "dockerfile",
            "makefile",
            "swift",
            "kotlin",
            "java",
            "c",
            "cpp",
            "text",
        ] {
            assert!(info(language).unwrap().badge.chars().count() <= 3);
        }
    }
}
