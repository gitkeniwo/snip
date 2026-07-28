#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageInfo {
    pub canonical_name: &'static str,
    pub extension: Option<&'static str>,
    pub badge: &'static str,
    pub aliases: &'static [&'static str],
}

const LANGUAGES: &[LanguageInfo] = &[
    LanguageInfo::new("Bash", Some("sh"), "sh", &["bash"]),
    LanguageInfo::new("Shell", Some("sh"), "sh", &["shell", "sh"]),
    LanguageInfo::new("Fish", Some("fish"), "fsh", &["fish"]),
    LanguageInfo::new("Zsh", Some("zsh"), "zsh", &["zsh"]),
    LanguageInfo::new(
        "PowerShell",
        Some("ps1"),
        "ps",
        &["powershell", "pwsh", "ps1"],
    ),
    LanguageInfo::new("Python", Some("py"), "py", &["python", "python3", "py"]),
    LanguageInfo::new("Ruby", Some("rb"), "rb", &["ruby", "rb"]),
    LanguageInfo::new("Perl", Some("pl"), "pl", &["perl", "pl"]),
    LanguageInfo::new("Lua", Some("lua"), "lua", &["lua"]),
    LanguageInfo::new("R", Some("r"), "r", &["r", "rscript"]),
    LanguageInfo::new("AWK", Some("awk"), "awk", &["awk", "gawk"]),
    LanguageInfo::new("Tcl", Some("tcl"), "tcl", &["tcl"]),
    LanguageInfo::new(
        "JavaScript",
        Some("js"),
        "js",
        &["javascript", "js", "node"],
    ),
    LanguageInfo::new("TypeScript", Some("ts"), "ts", &["typescript", "ts"]),
    LanguageInfo::new("JSX", Some("jsx"), "jsx", &["jsx", "react"]),
    LanguageInfo::new("TSX", Some("tsx"), "tsx", &["tsx", "react-typescript"]),
    LanguageInfo::new("Vue", Some("vue"), "vue", &["vue", "vuejs"]),
    LanguageInfo::new("Svelte", Some("svelte"), "svt", &["svelte"]),
    LanguageInfo::new("Elm", Some("elm"), "elm", &["elm"]),
    LanguageInfo::new("PureScript", Some("purs"), "pur", &["purescript", "purs"]),
    LanguageInfo::new("HTML", Some("html"), "htm", &["html", "htm"]),
    LanguageInfo::new("CSS", Some("css"), "css", &["css"]),
    LanguageInfo::new("SCSS", Some("scss"), "scs", &["scss"]),
    LanguageInfo::new("Sass", Some("sass"), "sas", &["sass"]),
    LanguageInfo::new("XML", Some("xml"), "xml", &["xml"]),
    LanguageInfo::new("GraphQL", Some("graphql"), "gql", &["graphql", "gql"]),
    LanguageInfo::new("JSON", Some("json"), "jsn", &["json"]),
    LanguageInfo::new("YAML", Some("yaml"), "yml", &["yaml", "yml"]),
    LanguageInfo::new("TOML", Some("toml"), "tml", &["toml"]),
    LanguageInfo::new("INI", Some("ini"), "ini", &["ini", "cfg"]),
    LanguageInfo::new("CSV", Some("csv"), "csv", &["csv"]),
    LanguageInfo::new(
        "Properties",
        Some("properties"),
        "prp",
        &["properties", "props"],
    ),
    LanguageInfo::new("HCL", Some("hcl"), "hcl", &["hcl"]),
    LanguageInfo::new("Terraform", Some("tf"), "tf", &["terraform", "tf"]),
    LanguageInfo::new("Nix", Some("nix"), "nix", &["nix", "nixos"]),
    LanguageInfo::new("Protobuf", Some("proto"), "pbf", &["protobuf", "proto"]),
    LanguageInfo::new("Dockerfile", None, "dkr", &["dockerfile", "docker"]),
    LanguageInfo::new("Makefile", None, "mk", &["makefile", "make", "gnumake"]),
    LanguageInfo::new("CMake", Some("cmake"), "cmk", &["cmake"]),
    LanguageInfo::new("Nginx", Some("nginx"), "ngx", &["nginx"]),
    LanguageInfo::new("Markdown", Some("md"), "md", &["markdown", "md"]),
    LanguageInfo::new(
        "reStructuredText",
        Some("rst"),
        "rst",
        &["restructuredtext", "rst"],
    ),
    LanguageInfo::new("LaTeX", Some("tex"), "tex", &["latex", "tex"]),
    LanguageInfo::new("BibTeX", Some("bib"), "bib", &["bibtex", "bib"]),
    LanguageInfo::new("Diff", Some("diff"), "dif", &["diff", "patch"]),
    LanguageInfo::new("SQL", Some("sql"), "sql", &["sql"]),
    LanguageInfo::new("Rust", Some("rs"), "rs", &["rust", "rs"]),
    LanguageInfo::new("Go", Some("go"), "go", &["go", "golang"]),
    LanguageInfo::new("Zig", Some("zig"), "zig", &["zig"]),
    LanguageInfo::new("Nim", Some("nim"), "nim", &["nim"]),
    LanguageInfo::new("Crystal", Some("cr"), "cry", &["crystal", "cr"]),
    LanguageInfo::new("D", Some("d"), "d", &["d", "dlang"]),
    LanguageInfo::new("C", Some("c"), "c", &["c"]),
    LanguageInfo::new("C++", Some("cpp"), "cpp", &["cpp", "c++", "cxx"]),
    LanguageInfo::new("C#", Some("cs"), "cs", &["csharp", "c#", "cs"]),
    LanguageInfo::new("F#", Some("fs"), "fs", &["fsharp", "f#", "fs"]),
    LanguageInfo::new("Java", Some("java"), "jav", &["java"]),
    LanguageInfo::new("Kotlin", Some("kt"), "kt", &["kotlin", "kt"]),
    LanguageInfo::new("Scala", Some("scala"), "scl", &["scala"]),
    LanguageInfo::new("Groovy", Some("groovy"), "grv", &["groovy"]),
    LanguageInfo::new("Clojure", Some("clj"), "clj", &["clojure", "clj"]),
    LanguageInfo::new("Swift", Some("swift"), "swf", &["swift"]),
    LanguageInfo::new("Dart", Some("dart"), "drt", &["dart"]),
    LanguageInfo::new("Haskell", Some("hs"), "hs", &["haskell", "hs"]),
    LanguageInfo::new("OCaml", Some("ml"), "ml", &["ocaml", "ml"]),
    LanguageInfo::new("Elixir", Some("ex"), "ex", &["elixir", "ex", "exs"]),
    LanguageInfo::new("Erlang", Some("erl"), "erl", &["erlang", "erl"]),
    LanguageInfo::new("Julia", Some("jl"), "jl", &["julia", "jl"]),
    LanguageInfo::new("MATLAB", Some("m"), "mat", &["matlab"]),
    LanguageInfo::new("Solidity", Some("sol"), "sol", &["solidity", "sol"]),
    LanguageInfo::new("Assembly", Some("asm"), "asm", &["assembly", "asm"]),
    LanguageInfo::new("Verilog", Some("v"), "ver", &["verilog"]),
    LanguageInfo::new("VHDL", Some("vhd"), "vhd", &["vhdl", "vhd"]),
    LanguageInfo::new(
        "Plain Text",
        None,
        "txt",
        &["text", "plain", "plaintext", ""],
    ),
];

pub fn all() -> &'static [LanguageInfo] {
    LANGUAGES
}

pub fn info(language: &str) -> Option<LanguageInfo> {
    let language = language.trim();
    LANGUAGES
        .iter()
        .find(|info| info.canonical_name.eq_ignore_ascii_case(language))
        .or_else(|| {
            LANGUAGES.iter().find(|info| {
                info.aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(language))
            })
        })
        .or_else(|| {
            LANGUAGES.iter().find(|info| {
                info.extension
                    .is_some_and(|extension| extension.eq_ignore_ascii_case(language))
            })
        })
        .copied()
}

impl LanguageInfo {
    const fn new(
        canonical_name: &'static str,
        extension: Option<&'static str>,
        badge: &'static str,
        aliases: &'static [&'static str],
    ) -> Self {
        Self {
            canonical_name,
            extension,
            badge,
            aliases,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn shell_languages_remain_visually_and_structurally_distinct() {
        assert_eq!(info("sh").unwrap().badge, "sh");
        assert_eq!(info("fish").unwrap().badge, "fsh");
        assert_eq!(info("fish").unwrap().extension, Some("fish"));
        assert_eq!(info("zsh").unwrap().badge, "zsh");
    }

    #[test]
    fn aliases_extensions_and_canonical_names_resolve_to_one_entry() {
        assert_eq!(info("TypeScript"), info("ts"));
        assert_eq!(info("YAML"), info("yml"));
        assert_eq!(info("C++"), info("cxx"));
        assert_eq!(info("PowerShell"), info("ps1"));
        assert_eq!(info("Plain Text"), info(""));
    }

    #[test]
    fn every_known_badge_collision_is_documented_and_fits_the_three_cell_column() {
        let mut badges = BTreeMap::<&str, Vec<&str>>::new();
        for language in all() {
            assert!(
                language.badge.chars().count() <= 3,
                "{} has an oversized badge",
                language.canonical_name
            );
            badges
                .entry(language.badge)
                .or_default()
                .push(language.canonical_name);
        }
        let collisions = badges
            .into_iter()
            .filter(|(_, languages)| languages.len() > 1)
            .collect::<Vec<_>>();
        assert_eq!(collisions, vec![("sh", vec!["Bash", "Shell"])]);
    }

    #[test]
    fn every_non_plain_language_has_real_highlighting() {
        let syntaxes = two_face::syntax::extra_newlines();
        let plain = syntaxes.find_syntax_plain_text().name.clone();
        for language in all()
            .iter()
            .filter(|language| language.canonical_name != "Plain Text")
        {
            let file = language.extension.map_or_else(
                || language.canonical_name.to_owned(),
                |extension| format!("fragment.{extension}"),
            );
            let syntax = crate::render::find_syntax(&syntaxes, language.aliases[0], file.as_str());
            assert_ne!(
                syntax.name, plain,
                "{} resolves to Plain Text",
                language.canonical_name
            );
        }
    }
}
