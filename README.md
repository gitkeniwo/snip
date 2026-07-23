# snip

<img width="887" height="629" alt="Screenshot 2026-07-23 at 22 08 34" src="https://github.com/user-attachments/assets/29012f73-975e-4d84-889c-8b85820487eb" />

`snip` is a filesystem-native snippet library for humans, shell scripts, and AI
agents. Markdown, source code, notes, and metadata remain ordinary files that
can be opened with any text editor. The CLI adds validation, structured JSON,
search, previews, optimistic concurrency, recoverable deletion, and import from
SnippetsLab.

The optional Ratatui browser provides a three-pane folder/tag, snippet, and
preview workflow. It searches interactively, edits through an external editor,
copies to the clipboard, and refreshes when CLI agents or text editors change
files on disk:

```bash
cargo run -- --library ./Main.sniplib tui
# With default_library configured and an interactive terminal:
cargo run --
```

Use `/` to search, `Tab` to cycle panes, `h`/`←` and `l`/`→` to move back or
drill in, and `j`/`k` to navigate. Moving through folders or tags filters the
snippet list immediately. The two-tone pill top bar shows the active path,
sort, list position, and fragment position. The matching pill-style bottom bar
keeps global navigation/search/help on the left and pane-specific editing
actions on the right; active search, status, or modal input temporarily takes
over the row.

The TUI provides complete local management, using the same words as the CLI:
`n` creates snippets or folders; `e`/`E`/`R` edit content, note, or README; `v`
opens in VS Code (`snip open`); `r` renames a snippet, or a folder within its
parent (`snip folder rename`); `m` moves a snippet, or reparents a folder
(`snip folder move`); `t` edits tags; `p`/`L` toggle pin or lock; and `d` moves
snippets to trash. `T` opens the
restore/purge view, `s` changes sort order, `F5` or `Ctrl-r` rescans, and `?`
shows the full key map. Preview source lines are numbered by default; `N`
toggles line numbers. Mouse click, double-click, fragment-tab click, and wheel
scrolling are supported. Dragging across Preview text selects it; releasing the
mouse copies the selection automatically, excluding the line-number gutter.
The TUI is enabled by default; a slim agent build is available with
`cargo build --no-default-features`.

On macOS, the TUI follows the system light/dark appearance and updates while it
is running. Linux uses `GTK_THEME` or `COLORFGBG` when available. Override
automatic detection for a terminal whose background differs from the system:

```bash
SNIP_TUI_THEME=light snip
SNIP_TUI_THEME=dark snip
```

Snippet titles use portable language badges such as `[rs]`, `[py]`, `[sh]`, and
`[md]`; these content icons deliberately avoid private-use glyphs. The top and
bottom pill chrome uses the Powerline round-cap glyphs included in Nerd Fonts
and other Powerline-patched terminal fonts. Custom file and folder icons can be
added later without changing the library format.

SQLite is not the source of truth. A future search cache may live under
`.snip/cache/`, but deleting that directory must never lose library data.

## Build

Rust 1.89 or newer is recommended. Dependencies are pinned in `Cargo.lock`.

```bash
cd /path/to/snip
cargo build --release
```

The binary is `target/release/snip`.

## Testing and CI

The repository uses three GitHub Actions workflows:

- `CI` runs on pushes and pull requests. It checks formatting, Clippy, the full
  default build on Linux and Apple Silicon macOS, Rust 1.89 compatibility, and
  the slim `--no-default-features` agent build.
- `Deep tests` is manually dispatched for the complete deterministic suite, the
  synthetic SnippetsLab importer fixture, the recursive-watcher regression, or
  an LCOV coverage report.
- `Release build` runs for `v*` tags and manual dispatch, producing tarballs for
  Linux x86_64/arm64, macOS arm64, and macOS Intel.

Run the equivalent local checks before pushing:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --no-default-features
cargo build --locked --release --all-features
```

For a local development library, bind `./Main.sniplib` as the default:

```bash
./target/release/snip config set default-library ./Main.sniplib
./target/release/snip info
```

`Main.sniplib/` is local working data and is ignored by the source repository.
It can be deleted and recreated with `snip init ./Main.sniplib --name Main`
without affecting the Rust project.

## Quick start

```bash
snip init ./Main.sniplib --name Main
snip config set default-library ./Main.sniplib

printf 'echo hello\n' | snip create \
  --title "Hello" \
  --folder Scripts/Shell \
  --tag demo \
  --language bash \
  --content-file -

snip list
snip list --sort modified          # manual | title | modified | created
snip search hello
snip preview Hello
snip edit Hello
snip open Hello                    # hand a managed path to an app, like the TUI's `v`
```

When `--library` is omitted, `SNIP_LIBRARY` is checked next, followed by walking
from the current directory toward the filesystem root for `snip.toml`, and
finally `default_library` in the user config.

## User configuration

The config lives at `$XDG_CONFIG_HOME/snip/config.toml`, or
`~/.config/snip/config.toml` when `XDG_CONFIG_HOME` is unset. Create it and bind
a default library with:

```bash
snip config init --library /path/to/Main.sniplib
snip config show
snip config path
```

Replace `/path/to/Main.sniplib` with the library you want to use by default.

You can safely change supported values without editing TOML by hand:

```bash
snip config set default-library /path/to/Main.sniplib
snip config set output json
snip config set color auto
snip config set preview-render ansi
snip config set preview-pager false
snip config set editor 'nvim -f'
snip config set pager 'less -R'
snip config set default-language rust
snip config set default-folder Agents/Generated
snip config set default-tags 'ai,generated'
snip config set tui-theme auto
snip config set tui-sort modified
snip config set tui-icons ascii
snip config unset default-folder
```

The complete schema is:

```toml
schema_version = 1
default_library = "/path/to/Main.sniplib"
output = "human"             # human | json | jsonl
color = "auto"               # auto | always | never
preview_render = "ansi"      # ansi | plain | html
preview_pager = false
editor = "nvim -f"
pager = "less -R"
default_language = "text"
default_folder = ""
default_tags = ["personal"]

[tui]
theme = "auto"             # auto | light | dark
sort = "manual"            # manual | title | modified | created
icons = "ascii"            # ascii | nerd; nerd falls back to ascii in v2
```

`SNIP_TUI_THEME=light|dark` overrides `[tui].theme`. Unknown values under a
future `[tui.colors]` table are preserved when the config is rewritten; custom
palette consumption and actual Nerd Font glyphs are reserved for a later
version.

Config values are defaults only. Explicit CLI options override them. Library
resolution is `--library` → `SNIP_LIBRARY` → nearest ancestor library →
`default_library`, so commands run inside a library never jump unexpectedly to
the global default. Unknown TOML fields are preserved when `snip config set` or
`unset` rewrites the file, allowing future GUI settings to coexist.

## Agent-friendly operations

Use UUIDs from JSON output for deterministic operations. Human-readable titles
are accepted only when they identify exactly one snippet.

```bash
snip --output json list
snip --output json search terraform
snip --output json show 428ac138

snip edit 428ac138 \
  --content-file - \
  --if-hash 03ab... <<'EOF'
replacement content
EOF
```

Structured stdout is kept separate from errors. Exit codes are stable:

| Code | Meaning |
|---:|---|
| 0 | success |
| 1 | I/O or internal failure |
| 2 | invalid CLI usage |
| 3 | missing or ambiguous selector |
| 4 | lock or fingerprint conflict |
| 5 | invalid library data |

`--output jsonl` emits one JSON value per line for lists and search results.
`cat` always emits only the raw fragment content.

## Files are the database

```text
Main.sniplib/
├── snip.toml
├── tags.toml
├── snippets/
│   └── Dotfiles/
│       └── Brewfile--a5792745/
│           ├── snippet.toml
│           ├── README.md
│           ├── fragments/001-Brewfile
│           ├── notes/001.md
│           └── attachments/
├── trash/
└── .snip/
```

The physical path below `snippets/` is the folder hierarchy. A snippet package
is recognized by `snippet.toml`; its directory name is descriptive and can be
moved or renamed manually. UUIDs in the manifest remain the stable identity.
See [FORMAT.md](FORMAT.md) for the v1 format contract.

Direct editor changes are discovered on the next scan. CLI mutations use a
library lock and atomic writes. `--if-hash` prevents an agent from overwriting a
version it did not read. `snip doctor --repair` recovers interrupted package
transactions.

## Preview and editing

```bash
snip preview ID --render ansi
snip preview ID --render plain
snip preview ID --render html > preview.html
snip preview ID --pager
```

`snip edit ID` copies the first fragment to a temporary file and opens the
configured `editor`, then `$VISUAL`, then `$EDITOR`, then `vi`. It checks the
original fingerprint before committing the result. Additional editor targets are available with
`--fragment`, `--note-editor`, `--readme-editor`, and `--metadata-editor`.

## SnippetsLab migration

The source library is opened read-only. Import is staged, validated, and only
then renamed to the requested destination.

```bash
snip import snippetslab \
  /path/to/main.snippetslablibrary \
  --into ./Main.sniplib \
  --dry-run

snip import snippetslab \
  /path/to/main.snippetslablibrary \
  --into ./Main.sniplib
```

The importer preserves snippet and fragment UUIDs, hierarchy, tags, flags,
timestamps, content, notes, and original lexer names. Attachments are reported
but their private SnippetsLab relationships are not imported in format v1.

## Git and deletion

Git is optional and normal writes never auto-commit. `snip init --git` creates a
dedicated repository. `snip git status`, `diff`, and `log` work for nested
libraries; `snip git commit` is deliberately restricted to a library that is
also the Git repository root.

`snip delete` moves packages into tracked `trash/`. `snip restore` moves them
back. Permanent deletion requires `snip purge SELECTOR --yes`.

## Shell completion

```bash
snip completion zsh > ~/.zfunc/_snip
snip completion fish > ~/.config/fish/completions/snip.fish
```

## Development checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```
