use clap::{ValueEnum, builder::PossibleValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Settable,
    FileOnly,
    Managed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigKey {
    DefaultLibrary,
    Output,
    Color,
    PreviewRender,
    PreviewPager,
    Editor,
    EditorCwd,
    Pager,
    DefaultLanguage,
    DefaultFolder,
    DefaultTags,
    TuiTheme,
    TuiLightTheme,
    TuiDarkTheme,
    TuiSort,
    TuiDensity,
    TuiLineNumbers,
    TuiSimplifiedUi,
    GitAutoCommitInterval,
    GitAutoPush,
    GitAutoPull,
    GitBackupOnQuit,
}

impl ConfigKey {
    pub const ALL: &'static [Self] = &[
        Self::DefaultLibrary,
        Self::Output,
        Self::Color,
        Self::PreviewRender,
        Self::PreviewPager,
        Self::Editor,
        Self::EditorCwd,
        Self::Pager,
        Self::DefaultLanguage,
        Self::DefaultFolder,
        Self::DefaultTags,
        Self::TuiTheme,
        Self::TuiLightTheme,
        Self::TuiDarkTheme,
        Self::TuiSort,
        Self::TuiDensity,
        Self::TuiLineNumbers,
        Self::TuiSimplifiedUi,
        Self::GitAutoCommitInterval,
        Self::GitAutoPush,
        Self::GitAutoPull,
        Self::GitBackupOnQuit,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::DefaultLibrary => "default-library",
            Self::Output => "output",
            Self::Color => "color",
            Self::PreviewRender => "preview-render",
            Self::PreviewPager => "preview-pager",
            Self::Editor => "editor",
            Self::EditorCwd => "editor-cwd",
            Self::Pager => "pager",
            Self::DefaultLanguage => "default-language",
            Self::DefaultFolder => "default-folder",
            Self::DefaultTags => "default-tags",
            Self::TuiTheme => "tui-theme",
            Self::TuiLightTheme => "tui-light-theme",
            Self::TuiDarkTheme => "tui-dark-theme",
            Self::TuiSort => "tui-sort",
            Self::TuiDensity => "tui-density",
            Self::TuiLineNumbers => "tui-line-numbers",
            Self::TuiSimplifiedUi => "tui-simplified-ui",
            Self::GitAutoCommitInterval => "git-auto-commit-interval",
            Self::GitAutoPush => "git-auto-push",
            Self::GitAutoPull => "git-auto-pull",
            Self::GitBackupOnQuit => "git-backup-on-quit",
        }
    }

    pub fn spec(self) -> &'static ConfigFieldSpec {
        CONFIG_FIELDS
            .iter()
            .find(|field| field.cli_key == Some(self))
            .expect("every ConfigKey has one field specification")
    }
}

impl ValueEnum for ConfigKey {
    fn value_variants<'a>() -> &'a [Self] {
        Self::ALL
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.name()).help(self.spec().summary))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigFieldSpec {
    pub toml_path: &'static str,
    pub cli_key: Option<ConfigKey>,
    pub kind: FieldKind,
    pub summary: &'static str,
    pub values: &'static str,
    pub example: &'static str,
}

const fn settable(
    toml_path: &'static str,
    cli_key: ConfigKey,
    summary: &'static str,
    values: &'static str,
    example: &'static str,
) -> ConfigFieldSpec {
    ConfigFieldSpec {
        toml_path,
        cli_key: Some(cli_key),
        kind: FieldKind::Settable,
        summary,
        values,
        example,
    }
}

pub const CONFIG_FIELDS: &[ConfigFieldSpec] = &[
    ConfigFieldSpec {
        toml_path: "schema_version",
        cli_key: None,
        kind: FieldKind::Managed,
        summary: "Configuration schema version managed by snip; do not edit it manually.",
        values: "1",
        example: "1",
    },
    settable(
        "default_library",
        ConfigKey::DefaultLibrary,
        "Selects the fallback library after command-line, environment, and ancestor discovery.",
        "path to a .sniplib directory",
        "\"/path/to/Main.sniplib\"",
    ),
    settable(
        "output",
        ConfigKey::Output,
        "Selects the default command output format.",
        "human | json | jsonl",
        "\"human\"",
    ),
    settable(
        "color",
        ConfigKey::Color,
        "Controls colored command output.",
        "auto | always | never",
        "\"auto\"",
    ),
    settable(
        "preview_render",
        ConfigKey::PreviewRender,
        "Selects the default preview renderer.",
        "ansi | plain | html",
        "\"ansi\"",
    ),
    settable(
        "preview_pager",
        ConfigKey::PreviewPager,
        "Controls whether previews use a pager.",
        "true | false",
        "false",
    ),
    settable(
        "editor",
        ConfigKey::Editor,
        "Sets the external editor command.",
        "non-empty command string",
        "\"nvim -f\"",
    ),
    settable(
        "editor_cwd",
        ConfigKey::EditorCwd,
        "Selects the working directory used by the external editor.",
        "inherit | library | folder | snippet | fragment",
        "\"inherit\"",
    ),
    ConfigFieldSpec {
        toml_path: "vscode_cmd",
        cli_key: None,
        kind: FieldKind::FileOnly,
        summary: "Sets the Visual Studio Code command used by open and TUI external-app actions.",
        values: "command string",
        example: "\"code\"",
    },
    settable(
        "pager",
        ConfigKey::Pager,
        "Sets the pager command used when PAGER is absent.",
        "non-empty command string",
        "\"less -R\"",
    ),
    settable(
        "default_language",
        ConfigKey::DefaultLanguage,
        "Sets the language assigned by create when no language is supplied.",
        "non-empty language name",
        "\"text\"",
    ),
    settable(
        "default_folder",
        ConfigKey::DefaultFolder,
        "Sets the folder assigned by create; an empty value means Uncategorized.",
        "folder path or empty string",
        "\"\"",
    ),
    settable(
        "default_tags",
        ConfigKey::DefaultTags,
        "Sets the tags assigned by create when no tags are supplied.",
        "comma-separated tags for the CLI; string array in TOML",
        "[\"personal\"]",
    ),
    settable(
        "tui.theme",
        ConfigKey::TuiTheme,
        "Selects automatic, light, dark, or named TUI theme resolution.",
        "auto | light | dark | theme name in TOML",
        "\"auto\"",
    ),
    settable(
        "tui.light_theme",
        ConfigKey::TuiLightTheme,
        "Selects the theme used for the light appearance slot.",
        "non-empty theme name",
        "\"light-default\"",
    ),
    settable(
        "tui.dark_theme",
        ConfigKey::TuiDarkTheme,
        "Selects the theme used for the dark appearance slot.",
        "non-empty theme name",
        "\"dark-default\"",
    ),
    settable(
        "tui.sort",
        ConfigKey::TuiSort,
        "Selects the default TUI snippet sort order.",
        "modified | created | title",
        "\"modified\"",
    ),
    settable(
        "tui.density",
        ConfigKey::TuiDensity,
        "Selects comfortable or compact TUI spacing.",
        "comfortable | compact",
        "\"comfortable\"",
    ),
    settable(
        "tui.line_numbers",
        ConfigKey::TuiLineNumbers,
        "Controls the preview line-number gutter.",
        "true | false",
        "true",
    ),
    settable(
        "tui.simplified_ui",
        ConfigKey::TuiSimplifiedUi,
        "Replaces Powerline caps with square, font-independent cells.",
        "true | false",
        "false",
    ),
    settable(
        "git.auto_commit_interval",
        ConfigKey::GitAutoCommitInterval,
        "Sets the automatic commit interval; zero disables scheduled commits and pushes.",
        "non-negative whole number of minutes",
        "0",
    ),
    settable(
        "git.auto_push",
        ConfigKey::GitAutoPush,
        "Pushes ahead commits after automatic interval work.",
        "true | false",
        "false",
    ),
    settable(
        "git.auto_pull",
        ConfigKey::GitAutoPull,
        "Fetches and integrates the upstream during configured TUI startup behavior.",
        "true | false",
        "false",
    ),
    settable(
        "git.backup_on_quit",
        ConfigKey::GitBackupOnQuit,
        "Requests an interactive backup before TUI exit.",
        "true | false",
        "false",
    ),
];

#[cfg(test)]
mod tests {
    use super::{CONFIG_FIELDS, ConfigKey, FieldKind};
    use crate::config::{AppConfig, GitConfig, TuiConfig};
    use std::fmt::Write as _;

    #[test]
    fn registry_paths_deserialize_without_using_extension_buckets() {
        let mut document = String::new();
        let mut table = "";
        for field in CONFIG_FIELDS {
            let (field_table, key) = field
                .toml_path
                .rsplit_once('.')
                .unwrap_or(("", field.toml_path));
            if field_table != table {
                table = field_table;
                let _ = writeln!(document, "\n[{table}]");
            }
            let _ = writeln!(document, "{key} = {}", field.example);
        }

        let parsed: AppConfig =
            toml::from_str(&document).expect("registry examples form valid config TOML");
        assert!(parsed.extra.is_empty());
        assert!(parsed.tui.as_ref().is_some_and(|tui| tui.extra.is_empty()));
        assert!(parsed.git.as_ref().is_some_and(|git| git.extra.is_empty()));
    }

    #[test]
    fn every_config_field_is_registered() {
        let AppConfig {
            schema_version: _,
            default_library: _,
            output: _,
            color: _,
            preview_render: _,
            preview_pager: _,
            editor: _,
            editor_cwd: _,
            vscode_cmd: _,
            pager: _,
            default_language: _,
            default_folder: _,
            default_tags: _,
            tui: _,
            git: _,
            extra: _,
        } = AppConfig::default();
        let TuiConfig {
            theme: _,
            light_theme: _,
            dark_theme: _,
            sort: _,
            density: _,
            line_numbers: _,
            simplified_ui: _,
            extra: _,
        } = TuiConfig::default();
        let GitConfig {
            auto_commit_interval: _,
            auto_push: _,
            auto_pull: _,
            backup_on_quit: _,
            extra: _,
        } = GitConfig::default();

        assert_eq!(CONFIG_FIELDS.len(), 24);
        assert_eq!(
            CONFIG_FIELDS
                .iter()
                .filter(|field| field.kind == FieldKind::Settable)
                .count(),
            ConfigKey::ALL.len()
        );
        assert_eq!(
            CONFIG_FIELDS
                .iter()
                .filter(|field| field.kind == FieldKind::FileOnly)
                .count(),
            1
        );
        assert_eq!(
            CONFIG_FIELDS
                .iter()
                .filter(|field| field.kind == FieldKind::Managed)
                .count(),
            1
        );
        for key in ConfigKey::ALL {
            assert_eq!(
                CONFIG_FIELDS
                    .iter()
                    .filter(|field| field.cli_key == Some(*key))
                    .count(),
                1,
                "{} must have exactly one field specification",
                key.name()
            );
        }
    }
}
