%%DESCRIPTION
snip user configuration is TOML schema version 1. It supplies machine-local defaults across libraries; it never travels as part of a .sniplib directory. Unknown fields are preserved when snip config set or unset rewrites the file.

%%LOCATION
The file is $XDG_CONFIG_HOME/snip/config.toml, or ~/.config/snip/config.toml when XDG_CONFIG_HOME is unset. Explicit command-line options override configuration. SNIP_TUI_THEME and an in-session appearance override may override theme selection.

%%SCHEMA
The complete schema and representative defaults are:

    schema_version = 1
    default_library = "/path/to/Main.sniplib"
    output = "human"
    color = "auto"
    preview_render = "ansi"
    preview_pager = false
    editor = "nvim -f"
    editor_cwd = "inherit"
    vscode_cmd = "code"
    pager = "less -R"
    default_language = "text"
    default_folder = ""
    default_tags = ["personal"]

    [tui]
    theme = "auto"
    light_theme = "light-default"
    dark_theme = "dark-default"
    sort = "modified"
    density = "comfortable"
    line_numbers = true
    simplified_ui = false

    [git]
    auto_commit_interval = 0
    auto_push = false
    auto_pull = false
    backup_on_quit = false

%%GENERAL KEYS
default_library is the fallback after --library, SNIP_LIBRARY, and ancestor discovery. output is human, json, or jsonl. color is auto, always, or never. preview_render is ansi, plain, or html. preview_pager controls pager use.

editor is the external editor command. vscode_cmd defaults to code and is used by open or the TUI's external-app action. pager defaults to less -R when PAGER is absent.

default_language, default_folder, and default_tags seed create when corresponding flags are absent. The empty folder means Uncategorized.

%%EDITOR WORKING DIRECTORY
editor_cwd is inherit, library, folder, snippet, or fragment. inherit keeps the caller directory. library uses the .sniplib root. folder uses the package's containing folder. snippet uses the package. fragment uses the fragment or note directory; README and metadata stay at package level.

Snippet packages are transactionally replaced as whole directories. Prefer folder or library when another process may write while an editor is open. On Windows, configure an absolute editor executable for untrusted libraries; relative commands are resolved before changing directory.

%%TUI KEYS
tui.theme is auto, light, dark, or a theme name. light_theme and dark_theme select appearance slots. sort is modified, created, title, or manual where supported. density is comfortable or compact. line_numbers controls the preview gutter. simplified_ui uses square bar caps without Powerline glyphs.

SNIP_TUI_THEME=light or dark overrides tui.theme; any other non-empty value selects a named theme. A session appearance override takes precedence without rewriting the file.

%%GIT KEYS
git.auto_commit_interval is a whole number of minutes. Zero disables scheduled commits and pushes. auto_push pushes ahead commits in the background after interval work. auto_pull fetches and integrates the upstream during configured startup behavior. backup_on_quit requests an interactive backup before TUI exit.

Automatic Git work belongs to the TUI and configuration, not the library format. Enabling auto_push while the interval is zero has no effect and produces a warning.

%%SEE ALSO
snip(1), snip-config(1), snip-tui(1), snip-theme(5), snip-keys(5)
