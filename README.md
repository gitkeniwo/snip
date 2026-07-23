# snip

<img width="887" height="629" alt="Screenshot 2026-07-23 at 22 08 34" src="https://github.com/user-attachments/assets/29012f73-975e-4d84-889c-8b85820487eb" />

`snip` keeps a snippet library in plain files. Source code, Markdown notes, and
metadata stay as ordinary text you can open in any editor, grep, diff, and put
under version control.

It replaces SnippetsLab, which keeps its library in a database only that app can
read. That design makes the snippets hard to reach from anywhere else, and it
means edits made outside the app never show up inside it. Using the filesystem
instead removes the middleman: an editor, a shell script, and an AI agent can
all work on one library at the same time.

Letting several writers share a library is the interesting problem, and it is
what the CLI is built around. Every command can emit JSON, snippets are
addressed by UUID, and writes are guarded by a content fingerprint, so one
writer cannot silently overwrite work it never read. Deletion goes to a trash
directory rather than disappearing.

The bundled terminal browser covers the other half — reading, skimming, and
editing by hand. It watches the library, so a change an agent makes in one
terminal shows up in the other while you are looking at it.

```bash
snip tui                              # or plain `snip` in an interactive terminal
snip --library ./Main.sniplib tui
```

See [Terminal browser](#terminal-browser) for what it can do,
[Agent-friendly operations](#agent-friendly-operations) for the JSON and
concurrency contract, and [FORMAT.md](FORMAT.md) for the on-disk format.

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

## Terminal browser

Three panes: folders and tags on the left, the snippet list in the middle, a
preview on the right. Selecting a folder or tag filters the list as you move.

| Key | |
|---|---|
| `/` | search; `Tab` cycles panes |
| `h`/`←`, `l`/`→` | back out, or drill in |
| `j`/`k` | move; `[`/`]` switch fragments |
| `n` | create a snippet, or a folder from the sidebar |
| `e`, `E`, `R` | edit content, note, or README in `$EDITOR` |
| `v` | open in VS Code (`snip open`) |
| `r` | rename a snippet, or the selected folder or tag |
| `m` | move a snippet to a folder, or reparent the selected folder |
| `t`, `p`, `L` | edit tags, toggle pin, toggle lock |
| `d`, `T` | move to trash; open the restore/purge view |
| `s`, `N` | change sort order; toggle preview line numbers |
| `F5`, `Ctrl-r` | rescan now (the watcher usually does this for you) |
| `?` | the full key map |

Keys are named after the CLI commands they run, so `r` on a folder is
`snip folder rename` and `m` is `snip folder move`.

The mouse works too: click to focus and select, double-click to drill in, click
a fragment tab, and scroll the pane under the cursor. Dragging across the
preview selects text and releasing copies it, without the line-number gutter.

The TUI ships by default. For a smaller agent-only binary, build with
`cargo build --no-default-features`.

### Appearance

On macOS the TUI follows the system light/dark setting and updates while it
runs; Linux uses `GTK_THEME` or `COLORFGBG` when available. Override it for a
terminal whose background differs from the system:

```bash
SNIP_TUI_THEME=light snip     # or: snip config set tui-theme light
```

Language badges are plain ASCII (`[rs]`, `[py]`, `[sh]`, `[md]`) so they render
in any font. The rounded caps on the top and bottom bars are Powerline glyphs,
which need a Nerd Font or another Powerline-patched terminal font.

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
snip --output json list --folder Scripts        # includes Scripts/Shell
snip --output json list --folder Scripts --no-subfolders
snip --output json search terraform
snip --output json show 428ac138

# Search is structure-aware, so it replaces grep/rg over the library
snip --output json search 'kubectl (apply|rollout)' --regex
snip --output json search rollout --context 2      # surrounding lines
snip --output json search deploy --field title --field tag --limit 10

# Content, notes, and READMEs take an inline value or a file (- is stdin)
snip edit 428ac138 --content 'replacement content' --if-hash 03ab...
snip edit 428ac138 --content-file - --if-hash 03ab... <<'EOF'
replacement content
EOF
```

Search results carry the snippet's `fingerprint`, so a metadata change (retag,
move, rename, delete) can go straight from `search` to `--if-hash` without a
separate `show`. Replacing content still means reading the content first —
`--if-hash` proves nobody else edited the snippet, not that the change is right.

External editing (`snip edit` with no structured change, `--metadata-editor`,
`--readme-editor`, `--note-editor`) requires an interactive terminal and exits
with a usage error otherwise, so scripts fail fast instead of blocking on an
editor that can never appear.

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

### Installable agent skill

[`skills/snip`](skills/snip/SKILL.md) packages the above into a skill any agent
can load — vocabulary, selectors, JSON payload shapes, the `--if-hash` workflow,
and the on-disk format. Symlink it into an agent's skills directory:

```bash
mkdir -p ~/.claude/skills && ln -s "$PWD/skills/snip" ~/.claude/skills/snip
```

See [`skills/README.md`](skills/README.md) for other runtimes.

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
├── .snip/
└── .gitignore
```

The physical path below `snippets/` is the folder hierarchy. A snippet package
is recognized by `snippet.toml`; its directory name is descriptive and can be
moved or renamed manually. UUIDs in the manifest remain the stable identity.

[FORMAT.md](FORMAT.md) specifies all of this normatively — manifests, path
rules, the fingerprint algorithm, and the transaction protocol — so another tool
can read and write a library without going through snip. That specification is
what makes "your snippets are not locked in an app" a checkable claim rather
than a slogan.

Direct editor changes are discovered on the next scan. CLI mutations use a
library lock and atomic writes. `--if-hash` prevents an agent from overwriting a
version it did not read. `snip doctor --repair` recovers interrupted package
transactions.

Nothing under `.snip/` is user data — it holds locks and in-flight transactions,
and may later hold a search cache. Deleting it while snip is not running must
never lose anything from the library.

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
