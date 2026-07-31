use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use snip::domain::{FolderFilter, SearchField};
use snip::sort::SortMode;

#[derive(Parser, Debug)]
#[command(
    name = "snip",
    version,
    about = "Filesystem-native snippets for humans, scripts, and AI agents"
)]
pub struct Cli {
    /// Library root. Falls back to SNIP_LIBRARY, ancestor discovery, then config.
    #[arg(long, global = true, env = "SNIP_LIBRARY")]
    pub library: Option<PathBuf>,

    /// Structured output mode for commands that return records.
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputMode>,

    /// Color policy for terminal preview.
    #[arg(long, global = true, value_enum)]
    pub color: Option<ColorMode>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputMode {
    Human,
    Json,
    Jsonl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Open the interactive terminal browser.
    #[cfg(feature = "tui")]
    Tui,
    /// Inspect and modify ~/.config/snip/config.toml.
    Config(ConfigArgs),
    /// Create a new filesystem snippet library.
    Init(InitArgs),
    /// Show library metadata and counts.
    Info,
    /// List snippets without their full content.
    List(FilterArgs),
    /// Search titles, tags, notes, and fragment content.
    Search(SearchArgs),
    /// Show a complete snippet.
    Show(SelectorArgs),
    /// Print one fragment with no decorations.
    Cat(FragmentSelectorArgs),
    /// Render a snippet for terminal or HTML preview.
    Preview(PreviewArgs),
    /// Print a managed filesystem path.
    Path(PathArgs),
    /// Open a managed path in an external application.
    Open(OpenArgs),
    /// Create a snippet.
    Create(CreateArgs),
    /// Modify a snippet or launch an external editor.
    Edit(EditArgs),
    /// Manage snippet fragments.
    Fragment(FragmentArgs),
    /// Manage physical folder paths.
    Folder(FolderArgs),
    /// Rename or remove tags across snippets.
    Tag(TagArgs),
    /// List, inspect, and switch TUI color themes.
    Theme(ThemeArgs),
    /// Move a snippet to the library trash.
    Delete(DeleteArgs),
    /// List deleted snippets.
    Trash,
    /// Restore a deleted snippet.
    Restore(RestoreArgs),
    /// Permanently remove a trash entry.
    Purge(PurgeArgs),
    /// Validate the library and optionally recover interrupted transactions.
    Doctor(DoctorArgs),
    /// Normalize snippet package directory names.
    Organize(OrganizeArgs),
    /// Import another snippet format.
    Import(ImportArgs),
    /// Run Git operations scoped to this library.
    Git(GitArgs),
    /// Install, inspect, or export the embedded manual pages.
    Man(ManArgs),
    /// Generate shell completion code.
    Completion(CompletionArgs),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub name: Option<String>,
    /// Initialize a dedicated Git repository after creating the library.
    #[arg(long)]
    pub git: bool,
    /// Skip the interactive setup and use defaults.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct FilterArgs {
    /// Restrict to a folder and its subfolders. Pass "" for Uncategorized.
    #[arg(long)]
    pub folder: Option<String>,
    /// Restrict --folder to that folder alone, excluding its subfolders.
    #[arg(long, requires = "folder")]
    pub no_subfolders: bool,
    #[arg(long)]
    pub tag: Option<String>,
    /// Order of the listing. Pinned snippets always come first.
    #[arg(long, default_value = "modified")]
    pub sort: SortMode,
}

impl FilterArgs {
    pub fn folder_filter(&self) -> Option<FolderFilter<'_>> {
        self.folder
            .as_deref()
            .map(|folder| FolderFilter::new(folder, !self.no_subfolders))
    }
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    pub query: String,
    /// Treat QUERY as a regular expression. Case-insensitive; use (?-i) to opt out.
    #[arg(long)]
    pub regex: bool,
    /// Restrict to a folder and its subfolders. Pass "" for Uncategorized.
    #[arg(long)]
    pub folder: Option<String>,
    /// Restrict --folder to that folder alone, excluding its subfolders.
    #[arg(long, requires = "folder")]
    pub no_subfolders: bool,
    #[arg(long)]
    pub tag: Option<String>,
    /// Only search these parts of a snippet. Repeatable; defaults to all.
    #[arg(long = "field")]
    pub fields: Vec<SearchField>,
    /// Lines of surrounding context to include with each match.
    #[arg(long, short = 'C', default_value_t = 0, value_name = "N")]
    pub context: usize,
    /// Keep only the top N results after scoring.
    #[arg(long, short = 'm', value_name = "N")]
    pub limit: Option<usize>,
}

impl SearchArgs {
    pub fn folder_filter(&self) -> Option<FolderFilter<'_>> {
        self.folder
            .as_deref()
            .map(|folder| FolderFilter::new(folder, !self.no_subfolders))
    }
}

#[derive(Args, Debug)]
pub struct SelectorArgs {
    pub selector: String,
}

#[derive(Args, Debug)]
pub struct FragmentSelectorArgs {
    pub selector: String,
    /// 1-based index or fragment UUID prefix.
    #[arg(long)]
    pub fragment: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum RenderArg {
    Ansi,
    Plain,
    Html,
}

#[derive(Args, Debug)]
pub struct PreviewArgs {
    pub selector: String,
    #[arg(long, value_enum)]
    pub render: Option<RenderArg>,
    /// Send output through the configured pager or $PAGER.
    #[arg(long, conflicts_with = "no_pager")]
    pub pager: bool,
    /// Disable a pager enabled in the config file.
    #[arg(long, conflicts_with = "pager")]
    pub no_pager: bool,
}

#[derive(Args, Debug)]
pub struct PathArgs {
    pub selector: String,
    #[arg(long, conflicts_with_all = ["readme", "fragment"])]
    pub metadata: bool,
    #[arg(long, conflicts_with_all = ["metadata", "fragment"])]
    pub readme: bool,
    #[arg(long, conflicts_with_all = ["metadata", "readme"])]
    pub fragment: Option<String>,
}

/// Same target selection as `snip path`, but hands the resolved path to an app.
/// This is the CLI counterpart of the TUI's `v` key.
#[derive(Args, Debug)]
pub struct OpenArgs {
    pub selector: String,
    #[arg(long, conflicts_with_all = ["readme", "fragment"])]
    pub metadata: bool,
    #[arg(long, conflicts_with_all = ["metadata", "fragment"])]
    pub readme: bool,
    #[arg(long, conflicts_with_all = ["metadata", "readme"])]
    pub fragment: Option<String>,
    /// Command to launch. Defaults to the `vscode_cmd` config key, then `code`.
    #[arg(long)]
    pub app: Option<String>,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub folder: Option<String>,
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    #[arg(long)]
    pub language: Option<String>,
    #[arg(long, default_value = "Fragment")]
    pub fragment_title: String,
    /// Initial fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    /// Read initial fragment content from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub content_file: Option<String>,
    /// Fragment note (Markdown), given inline.
    #[arg(long, conflicts_with = "note_file")]
    pub note: Option<String>,
    #[arg(long)]
    pub note_file: Option<String>,
    /// Snippet README (Markdown), given inline.
    #[arg(long, conflicts_with = "readme_file")]
    pub readme: Option<String>,
    #[arg(long)]
    pub readme_file: Option<String>,
    #[arg(long)]
    pub pin: bool,
    #[arg(long)]
    pub lock: bool,
}

#[derive(Args, Debug, Clone)]
pub struct OptimisticArgs {
    #[arg(long)]
    pub if_hash: Option<String>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    pub selector: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub folder: Option<String>,
    #[arg(long = "tag", conflicts_with = "clear_tags")]
    pub tags: Vec<String>,
    #[arg(long, conflicts_with = "tags")]
    pub clear_tags: bool,
    #[arg(long, conflicts_with = "unpin")]
    pub pin: bool,
    #[arg(long, conflicts_with = "pin")]
    pub unpin: bool,
    #[arg(long, conflicts_with = "unlock")]
    pub lock: bool,
    #[arg(long, conflicts_with = "lock")]
    pub unlock: bool,
    /// Target fragment for structured changes or external editing.
    #[arg(long)]
    pub fragment: Option<String>,
    #[arg(long)]
    pub fragment_title: Option<String>,
    #[arg(long)]
    pub language: Option<String>,
    /// Replacement fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    #[arg(long)]
    pub content_file: Option<String>,
    /// Replacement fragment note (Markdown), given inline.
    #[arg(long, conflicts_with_all = ["note_file", "clear_note"])]
    pub note: Option<String>,
    #[arg(long, conflicts_with = "clear_note")]
    pub note_file: Option<String>,
    #[arg(long, conflicts_with_all = ["note_file", "note"])]
    pub clear_note: bool,
    /// Replacement snippet README (Markdown), given inline.
    #[arg(long, conflicts_with_all = ["readme_file", "clear_readme"])]
    pub readme: Option<String>,
    #[arg(long, conflicts_with = "clear_readme")]
    pub readme_file: Option<String>,
    #[arg(long, conflicts_with_all = ["readme_file", "readme"])]
    pub clear_readme: bool,
    /// Edit snippet.toml in the external editor when no structured change is given.
    #[arg(long, conflicts_with = "readme_editor")]
    pub metadata_editor: bool,
    /// Edit README.md in the external editor when no structured change is given.
    #[arg(long, conflicts_with = "metadata_editor")]
    pub readme_editor: bool,
    /// Edit the selected fragment note instead of its content.
    #[arg(long)]
    pub note_editor: bool,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentArgs {
    #[command(subcommand)]
    pub command: FragmentCommand,
}

#[derive(Subcommand, Debug)]
pub enum FragmentCommand {
    Add(FragmentAddArgs),
    Edit(FragmentEditArgs),
    Remove(FragmentRemoveArgs),
    Reorder(FragmentReorderArgs),
}

#[derive(Args, Debug)]
pub struct FragmentAddArgs {
    pub selector: String,
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub language: Option<String>,
    /// Fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    #[arg(long)]
    pub content_file: Option<String>,
    /// Fragment note (Markdown), given inline.
    #[arg(long, conflicts_with = "note_file")]
    pub note: Option<String>,
    #[arg(long)]
    pub note_file: Option<String>,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentEditArgs {
    pub selector: String,
    pub fragment: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub language: Option<String>,
    /// Replacement fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    #[arg(long)]
    pub content_file: Option<String>,
    /// Replacement fragment note (Markdown), given inline.
    #[arg(long, conflicts_with_all = ["note_file", "clear_note"])]
    pub note: Option<String>,
    #[arg(long, conflicts_with = "clear_note")]
    pub note_file: Option<String>,
    #[arg(long, conflicts_with_all = ["note_file", "note"])]
    pub clear_note: bool,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentRemoveArgs {
    pub selector: String,
    pub fragment: String,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentReorderArgs {
    pub selector: String,
    pub fragment: String,
    #[arg(long)]
    pub position: usize,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FolderArgs {
    #[command(subcommand)]
    pub command: FolderCommand,
}

#[derive(Subcommand, Debug)]
pub enum FolderCommand {
    List,
    Create { folder: String },
    Rename { folder: String, new_name: String },
    Move { folder: String, target: String },
    Delete { folder: String },
}

#[derive(Args, Debug)]
pub struct TagArgs {
    #[command(subcommand)]
    pub command: TagCommand,
}

#[derive(Subcommand, Debug)]
pub enum TagCommand {
    List,
    Rename { tag: String, new_name: String },
    Delete { tag: String },
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    pub selector: String,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct RestoreArgs {
    pub selector: String,
    #[arg(long)]
    pub folder: Option<String>,
}

#[derive(Args, Debug)]
pub struct PurgeArgs {
    pub selector: String,
    /// Confirm permanent deletion without an interactive prompt.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    #[arg(long)]
    pub repair: bool,
}

#[derive(Args, Debug)]
pub struct OrganizeArgs {
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    #[command(subcommand)]
    pub command: ImportCommand,
}

#[derive(Subcommand, Debug)]
pub enum ImportCommand {
    Snippetslab {
        source: PathBuf,
        #[arg(long)]
        into: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args, Debug)]
pub struct GitArgs {
    #[command(subcommand)]
    pub command: GitCommand,
}

#[derive(Subcommand, Debug)]
pub enum GitCommand {
    /// Show read-only backup status for this library.
    Status,
    /// Make this library a Git repository when it is not one already.
    Init,
    /// Commit all library changes without pushing.
    Commit {
        /// Override the generated backup message.
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Commit all library changes and push when an upstream is configured.
    Backup,
    /// Push commits to the configured upstream without committing.
    Push,
    /// Fetch and prune remote-tracking refs without touching the worktree.
    Fetch,
}

#[derive(Args, Debug)]
pub struct ManArgs {
    #[command(subcommand)]
    pub command: ManCommand,
}

#[derive(Subcommand, Debug)]
pub enum ManCommand {
    /// Print the resolved man page installation directory.
    Path {
        /// Resolve the directory below PREFIX/share instead of the user data directory.
        #[arg(long)]
        prefix: Option<PathBuf>,
    },
    /// Install all manual pages.
    Install {
        /// Install below PREFIX/share instead of the user data directory.
        #[arg(long)]
        prefix: Option<PathBuf>,
        /// Replace pages not recorded in snip's installation manifest.
        #[arg(long)]
        force: bool,
    },
    /// Remove unmodified pages recorded by snip.
    Uninstall {
        /// Uninstall below PREFIX/share instead of the user data directory.
        #[arg(long)]
        prefix: Option<PathBuf>,
    },
    /// Show an embedded manual page with the system man viewer.
    Show {
        /// Page stem, such as snip or snip-create.
        page: Option<String>,
    },
    /// Export all embedded manual pages to a directory.
    Generate {
        /// Destination directory.
        directory: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Powershell,
    Zsh,
}

#[derive(Args, Debug)]
pub struct CompletionArgs {
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Args, Debug)]
pub struct ThemeArgs {
    #[command(subcommand)]
    pub command: ThemeCommand,
}

#[derive(Subcommand, Debug)]
pub enum ThemeCommand {
    /// List available themes.
    List {
        /// Restrict to one appearance.
        #[arg(long, value_enum)]
        appearance: Option<AppearanceArg>,
    },
    /// Print one theme's resolved colors.
    Show { name: String },
    /// Validate a theme's contrast and role distinctness.
    Check { name: String },
    /// Print the directory user themes are read from.
    Path,
    /// Write a theme to the user theme directory so it can be edited.
    Export {
        name: String,
        /// Name to save it under. Defaults to "<name>-custom".
        #[arg(long)]
        r#as: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Select a theme for its appearance slot and save it to the config.
    Use {
        name: String,
        /// Write to this slot instead of the theme's own appearance.
        #[arg(long, value_enum)]
        appearance: Option<AppearanceArg>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "lowercase")]
pub enum AppearanceArg {
    Light,
    Dark,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Print the resolved config file path.
    Path,
    /// Print the current config (defaults to an empty schema when absent).
    Show,
    /// Create a config file, optionally binding a default library.
    Init {
        #[arg(long)]
        library: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    /// Set one supported config key.
    Set {
        #[arg(value_enum)]
        key: ConfigKey,
        value: String,
    },
    /// Remove one supported config key and restore its built-in default.
    Unset {
        #[arg(value_enum)]
        key: ConfigKey,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ConfigKey {
    DefaultLibrary,
    Output,
    Color,
    PreviewRender,
    PreviewPager,
    Editor,
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
    GitAutoCommitInterval,
    GitAutoPush,
    GitBackupOnQuit,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn clap_command_tree_is_valid() {
        Cli::command().debug_assert();
    }
}
