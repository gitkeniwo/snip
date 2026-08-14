use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use snip::config::ConfigKey;
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

    /// Use square TUI bars without Powerline font glyphs for this run.
    #[cfg(feature = "tui")]
    #[arg(
        long,
        global = true,
        num_args = 0..=1,
        default_missing_value = "true",
        require_equals = true,
        value_name = "BOOL"
    )]
    pub simplified_ui: Option<bool>,

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
    #[command(
        long_about = "Open the interactive terminal browser.\n\nTUI bindings are loaded from keys.toml beside config.toml. Run `snip keys list` to inspect the effective bindings, `snip keys export` to create an editable starting point, and `snip keys check` to validate changes."
    )]
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
    /// Inspect, validate, and export TUI key bindings.
    #[cfg(feature = "tui")]
    Keys(KeysArgs),
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
    /// Publish snippets to GitHub Gists through the gh CLI.
    Gist(GistArgs),
    /// Install, inspect, or export the embedded manual pages.
    Man(ManArgs),
    /// Generate shell completion code.
    Completion(CompletionArgs),
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Directory to create. Defaults to an interactive or platform-specific choice.
    pub path: Option<PathBuf>,
    /// Human-readable library name.
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
    /// Restrict results to snippets carrying this tag.
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
    /// Text or regular expression to find.
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
    /// Restrict results to snippets carrying this tag.
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
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
}

#[derive(Args, Debug)]
pub struct FragmentSelectorArgs {
    /// Snippet title, UUID prefix, or package path.
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
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Rendering format. Defaults to the configured preview format.
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
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Print the path to snippet.toml.
    #[arg(long, conflicts_with_all = ["readme", "fragment"])]
    pub metadata: bool,
    /// Print the path to README.md.
    #[arg(long, conflicts_with_all = ["metadata", "fragment"])]
    pub readme: bool,
    /// Print the path to a fragment selected by 1-based index or UUID prefix.
    #[arg(long, conflicts_with_all = ["metadata", "readme"])]
    pub fragment: Option<String>,
}

/// Same target selection as `snip path`, but hands the resolved path to an app.
/// This is the CLI counterpart of the TUI's `v` key.
#[derive(Args, Debug)]
pub struct OpenArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Open snippet.toml.
    #[arg(long, conflicts_with_all = ["readme", "fragment"])]
    pub metadata: bool,
    /// Open README.md.
    #[arg(long, conflicts_with_all = ["metadata", "fragment"])]
    pub readme: bool,
    /// Open a fragment selected by 1-based index or UUID prefix.
    #[arg(long, conflicts_with_all = ["metadata", "readme"])]
    pub fragment: Option<String>,
    /// Command to launch. Defaults to the `vscode_cmd` config key, then `code`.
    #[arg(long)]
    pub app: Option<String>,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Snippet title.
    #[arg(long)]
    pub title: String,
    /// Folder to file the snippet under; created when it does not exist. Pass "" for Uncategorized.
    #[arg(long)]
    pub folder: Option<String>,
    /// Tag to attach. Repeatable.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Syntax language for the initial fragment.
    #[arg(long)]
    pub language: Option<String>,
    /// Title for the initial fragment.
    #[arg(long, default_value = "Fragment")]
    pub fragment_title: String,
    /// Initial fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    /// Read initial fragment content from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub content_file: Option<String>,
    /// Fragment note (Markdown), given inline. Prose about this one fragment; use --readme for prose about the whole snippet.
    #[arg(long, conflicts_with = "note_file")]
    pub note: Option<String>,
    /// Read the fragment note from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub note_file: Option<String>,
    /// Snippet README (Markdown), given inline. Prose about the whole snippet; use --note for prose about one fragment.
    #[arg(long, conflicts_with = "readme_file")]
    pub readme: Option<String>,
    /// Read the snippet README from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub readme_file: Option<String>,
    /// Pin the new snippet.
    #[arg(long)]
    pub pin: bool,
    /// Lock the new snippet against mutation.
    #[arg(long)]
    pub lock: bool,
}

#[derive(Args, Debug, Clone)]
pub struct OptimisticArgs {
    /// Refuse the write unless the snippet still has this fingerprint.
    #[arg(long)]
    pub if_hash: Option<String>,
    /// Bypass fingerprint checks and locked-snippet protection.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Replace the snippet title.
    #[arg(long)]
    pub title: Option<String>,
    /// Move the snippet to this folder. Pass "" for Uncategorized.
    #[arg(long)]
    pub folder: Option<String>,
    /// Replace tags with this tag set. Repeatable.
    #[arg(long = "tag", conflicts_with = "clear_tags")]
    pub tags: Vec<String>,
    /// Remove every tag from the snippet.
    #[arg(long, conflicts_with = "tags")]
    pub clear_tags: bool,
    /// Pin the snippet.
    #[arg(long, conflicts_with = "unpin")]
    pub pin: bool,
    /// Unpin the snippet.
    #[arg(long, conflicts_with = "pin")]
    pub unpin: bool,
    /// Lock the snippet against mutation.
    #[arg(long, conflicts_with = "unlock")]
    pub lock: bool,
    /// Unlock the snippet.
    #[arg(long, conflicts_with = "lock")]
    pub unlock: bool,
    /// Target fragment for structured changes or external editing.
    #[arg(long)]
    pub fragment: Option<String>,
    /// Replace the selected fragment title.
    #[arg(long)]
    pub fragment_title: Option<String>,
    /// Replace the selected fragment syntax language.
    #[arg(long)]
    pub language: Option<String>,
    /// Replacement fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    /// Read replacement fragment content from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub content_file: Option<String>,
    /// Replacement fragment note (Markdown), given inline.
    #[arg(long, conflicts_with_all = ["note_file", "clear_note"])]
    pub note: Option<String>,
    /// Read the replacement fragment note from a UTF-8 file; use - for stdin.
    #[arg(long, conflicts_with = "clear_note")]
    pub note_file: Option<String>,
    /// Remove the selected fragment note.
    #[arg(long, conflicts_with_all = ["note_file", "note"])]
    pub clear_note: bool,
    /// Replacement snippet README (Markdown), given inline.
    #[arg(long, conflicts_with_all = ["readme_file", "clear_readme"])]
    pub readme: Option<String>,
    /// Read the replacement snippet README from a UTF-8 file; use - for stdin.
    #[arg(long, conflicts_with = "clear_readme")]
    pub readme_file: Option<String>,
    /// Remove the snippet README.
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
    /// Add a fragment to a snippet.
    Add(FragmentAddArgs),
    /// Modify a fragment.
    Edit(FragmentEditArgs),
    /// Remove a fragment.
    Remove(FragmentRemoveArgs),
    /// Move a fragment to a new 1-based position.
    Reorder(FragmentReorderArgs),
}

#[derive(Args, Debug)]
pub struct FragmentAddArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Fragment title.
    #[arg(long)]
    pub title: String,
    /// Syntax language for the fragment.
    #[arg(long)]
    pub language: Option<String>,
    /// Fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    /// Read fragment content from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub content_file: Option<String>,
    /// Fragment note (Markdown), given inline.
    #[arg(long, conflicts_with = "note_file")]
    pub note: Option<String>,
    /// Read the fragment note from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub note_file: Option<String>,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentEditArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Fragment 1-based index or UUID prefix.
    pub fragment: String,
    /// Replace the fragment title.
    #[arg(long)]
    pub title: Option<String>,
    /// Replace the fragment syntax language.
    #[arg(long)]
    pub language: Option<String>,
    /// Replacement fragment content, given inline.
    #[arg(long, conflicts_with = "content_file")]
    pub content: Option<String>,
    /// Read replacement fragment content from a UTF-8 file; use - for stdin.
    #[arg(long)]
    pub content_file: Option<String>,
    /// Replacement fragment note (Markdown), given inline.
    #[arg(long, conflicts_with_all = ["note_file", "clear_note"])]
    pub note: Option<String>,
    /// Read the replacement note from a UTF-8 file; use - for stdin.
    #[arg(long, conflicts_with = "clear_note")]
    pub note_file: Option<String>,
    /// Remove the fragment note.
    #[arg(long, conflicts_with_all = ["note_file", "note"])]
    pub clear_note: bool,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentRemoveArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Fragment 1-based index or UUID prefix.
    pub fragment: String,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct FragmentReorderArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Fragment 1-based index or UUID prefix.
    pub fragment: String,
    /// New 1-based position.
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
    /// List physical snippet folders.
    List,
    /// Create a folder path.
    Create {
        /// Folder path to create.
        folder: String,
    },
    /// Rename the final component of a folder path.
    Rename {
        /// Existing folder path.
        folder: String,
        /// New final path component.
        new_name: String,
    },
    /// Move a folder and its snippets.
    Move {
        /// Existing folder path.
        folder: String,
        /// Full destination folder path.
        target: String,
    },
    /// Delete an empty folder.
    Delete {
        /// Empty folder path to delete.
        folder: String,
    },
}

#[derive(Args, Debug)]
pub struct TagArgs {
    #[command(subcommand)]
    pub command: TagCommand,
}

#[derive(Subcommand, Debug)]
pub enum TagCommand {
    /// List all known tags.
    List,
    /// Rename a tag across the library.
    Rename {
        /// Existing tag name.
        tag: String,
        /// Replacement tag name.
        new_name: String,
    },
    /// Delete a tag from the registry and every snippet.
    Delete {
        /// Tag name to delete.
        tag: String,
    },
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    #[command(flatten)]
    pub optimistic: OptimisticArgs,
}

#[derive(Args, Debug)]
pub struct RestoreArgs {
    /// Trash entry ID or unique prefix.
    pub selector: String,
    /// Restore into this folder instead of the original location.
    #[arg(long)]
    pub folder: Option<String>,
}

#[derive(Args, Debug)]
pub struct PurgeArgs {
    /// Trash entry ID or unique prefix.
    pub selector: String,
    /// Confirm permanent deletion without an interactive prompt.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Finish or roll back interrupted filesystem transactions.
    #[arg(long)]
    pub repair: bool,
}

#[derive(Args, Debug)]
pub struct OrganizeArgs {
    /// Report package-directory renames without applying them.
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
    /// Import a SnippetsLab library or export.
    Snippetslab {
        /// SnippetsLab source file or directory.
        source: PathBuf,
        /// Destination snip library.
        #[arg(long)]
        into: PathBuf,
        /// Validate and report the import without writing it.
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
    /// Clone a snip library from a Git remote.
    Clone {
        /// Remote to clone. Passed to git unchanged; with --gh it is passed to
        /// gh, which also accepts an OWNER/REPO shorthand.
        remote: String,
        /// Destination directory. Defaults to ~/<repo name>.sniplib.
        path: Option<PathBuf>,
        /// Clone with `gh repo clone` instead of `git clone`, so GitHub
        /// credentials come from the GitHub CLI.
        #[arg(long)]
        gh: bool,
        /// Record the clone as default_library in the snip config.
        #[arg(long)]
        set_default: bool,
    },
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
    /// Fetch from the upstream and merge it into the current branch.
    Pull {
        /// Refuse to merge when the branch has diverged, instead of creating a
        /// merge commit.
        #[arg(long)]
        ff_only: bool,
    },
}

#[derive(Args, Debug)]
pub struct GistArgs {
    #[command(subcommand)]
    pub command: GistCommand,
}

#[derive(Subcommand, Debug)]
pub enum GistCommand {
    /// Create the snippet's gist, or update it when one already exists.
    Push(GistPushArgs),
    /// Print the URL of the snippet's gist.
    Url(GistUrlArgs),
    /// Report whether a snippet's gist is current.
    Status(GistStatusArgs),
    /// Record an existing gist as this snippet's gist.
    Attach(GistAttachArgs),
    /// Forget a snippet's gist without deleting it on GitHub.
    Detach(GistSelectorArgs),
    /// Delete the snippet's gist on GitHub and forget it.
    Delete(GistDeleteArgs),
    /// Open the snippet's gist in a browser.
    Open(GistSelectorArgs),
}

#[derive(Args, Debug)]
pub struct GistSelectorArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
}

#[derive(Args, Debug)]
pub struct GistPushArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// List the gist publicly. Only applies when creating; gist visibility
    /// cannot be changed afterwards.
    #[arg(long)]
    pub public: bool,
    /// Gist description. Defaults to the snippet title, then to the
    /// description recorded by the previous push.
    #[arg(long)]
    pub desc: Option<String>,
    /// Publish a new gist and replace the recorded one, leaving the old gist
    /// on GitHub.
    #[arg(long)]
    pub new: bool,
    /// Publish fragment notes as separate Markdown files.
    #[arg(long)]
    pub include_notes: bool,
    /// Leave the snippet README out of the gist.
    #[arg(long)]
    pub no_readme: bool,
    /// Open the gist in a browser afterwards.
    #[arg(short, long)]
    pub web: bool,
    /// Refuse the push unless the snippet still has this fingerprint.
    #[arg(long)]
    pub if_hash: Option<String>,
    /// Push even when the gist already matches the snippet.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct GistUrlArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Copy the URL to the clipboard as well as printing it.
    #[arg(long)]
    pub copy: bool,
}

#[derive(Args, Debug)]
pub struct GistStatusArgs {
    /// Snippet title, UUID prefix, or package path.
    #[arg(conflicts_with = "all")]
    pub selector: Option<String>,
    /// Report every snippet in the library that has a gist.
    #[arg(long)]
    pub all: bool,
    /// Also fetch the gist from GitHub and report whether it still exists.
    #[arg(long)]
    pub remote: bool,
}

#[derive(Args, Debug)]
pub struct GistAttachArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Gist ID or URL.
    pub gist: String,
}

#[derive(Args, Debug)]
pub struct GistDeleteArgs {
    /// Snippet title, UUID prefix, or package path.
    pub selector: String,
    /// Delete without prompting.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct ManArgs {
    #[command(subcommand)]
    pub command: ManCommand,
}

#[derive(Subcommand, Debug)]
pub enum ManCommand {
    /// Print the resolved man hierarchy root containing man1, man5, and man7.
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
        /// Page stem or explicit page, such as snip, sniplib, or config.5.
        page: Option<String>,
    },
    /// Export all embedded manual pages to a directory.
    Generate {
        /// Destination root; pages are written below man1, man5, and man7.
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
    /// Shell whose completion script should be generated.
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

#[derive(Args, Debug)]
pub struct KeysArgs {
    #[command(subcommand)]
    pub command: KeysCommand,
}

#[derive(Subcommand, Debug)]
pub enum KeysCommand {
    /// List effective bindings, grouped by mode.
    List {
        /// Restrict output to one mode.
        #[arg(long, value_enum)]
        mode: Option<KeyModeArg>,
    },
    /// Show every mode and chord bound to one action slug.
    Show {
        /// Action slug to inspect.
        action: String,
    },
    /// Print the resolved keys.toml path.
    Path,
    /// Write the built-in bindings as an authoritative keys.toml.
    Export {
        /// Export only one mode.
        #[arg(long, value_enum)]
        mode: Option<KeyModeArg>,
        /// Overwrite an existing keys.toml.
        #[arg(long)]
        force: bool,
    },
    /// Validate keys.toml and report strict binding diagnostics.
    Check,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum KeyModeArg {
    Global,
    Sidebar,
    List,
    Preview,
    Fragment,
    FragmentGrab,
    Trash,
    Help,
    Git,
    Gist,
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
    Show {
        /// Theme name.
        name: String,
    },
    /// Validate a theme's contrast and role distinctness.
    Check {
        /// Theme name.
        name: String,
    },
    /// Print the directory user themes are read from.
    Path,
    /// Write a theme to the user theme directory so it can be edited.
    Export {
        /// Theme name to export.
        name: String,
        /// Name to save it under. Defaults to "<name>-custom".
        #[arg(long)]
        r#as: Option<String>,
        /// Overwrite an existing exported theme.
        #[arg(long)]
        force: bool,
    },
    /// Convert a base16 scheme file into a user theme.
    Import {
        /// Path to a base16 or base24 scheme file, or "-" to read stdin.
        path: PathBuf,
        /// Name to save it under. Defaults to the file stem.
        #[arg(long)]
        r#as: Option<String>,
        /// Use an embedded syntax theme instead of deriving one from the palette.
        #[arg(long)]
        syntax: Option<String>,
        /// Overwrite an existing theme file.
        #[arg(long)]
        force: bool,
        /// Convert and print the theme without writing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Select a theme for its appearance slot and save it to the config.
    Use {
        /// Theme name to select.
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
        /// Default library path to store in the new config.
        #[arg(long)]
        library: Option<PathBuf>,
        /// Replace an existing config file.
        #[arg(long)]
        force: bool,
    },
    /// Set one supported config key.
    Set {
        /// Configuration key to update.
        #[arg(value_enum)]
        key: ConfigKey,
        /// New value, parsed according to the selected key.
        value: String,
    },
    /// Remove one supported config key and restore its built-in default.
    Unset {
        /// Configuration key to remove.
        #[arg(value_enum)]
        key: ConfigKey,
    },
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;
    #[cfg(feature = "tui")]
    use clap::Parser;

    #[test]
    fn clap_command_tree_is_valid() {
        Cli::command().debug_assert();
    }

    #[cfg(feature = "tui")]
    #[test]
    fn simplified_ui_is_an_optional_global_boolean_override() {
        let enabled = Cli::try_parse_from(["snip", "--simplified-ui"]).unwrap();
        assert_eq!(enabled.simplified_ui, Some(true));

        let disabled = Cli::try_parse_from(["snip", "--simplified-ui=false"]).unwrap();
        assert_eq!(disabled.simplified_ui, Some(false));

        let after_subcommand = Cli::try_parse_from(["snip", "tui", "--simplified-ui"]).unwrap();
        assert_eq!(after_subcommand.simplified_ui, Some(true));

        let before_subcommand = Cli::try_parse_from(["snip", "--simplified-ui", "tui"]).unwrap();
        assert_eq!(before_subcommand.simplified_ui, Some(true));

        let absent = Cli::try_parse_from(["snip", "tui"]).unwrap();
        assert_eq!(absent.simplified_ui, None);
    }
}
