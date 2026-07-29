# snip

<img width="993" height="657" alt="image" src="https://github.com/user-attachments/assets/9d72c109-f3bb-4b35-b0d4-436c2220e2ad" />

`snip` is a snippet manager that keeps its library in plain files. Code, notes,
and metadata are ordinary text you can grep, diff, edit in any editor, and put
under Git. It ships as a CLI and a terminal browser.

It grew out of wanting SnippetsLab's library available while coding with an AI
agent. SnippetsLab stores everything in a database only that app can read, so
nothing else — a script, an editor, an agent — can reach it. Files remove that
limitation: every command speaks JSON, snippets are addressed by UUID, and
writes are guarded by a content fingerprint so two writers can share one library
without clobbering each other.

Runs on Linux, macOS, and Windows.

## Install

Homebrew, on macOS or Linux:

```bash
brew install gitkeniwo/snip/snip
```

From crates.io (the crate is `sniplab`; the binary it installs is `snip`):

```bash
cargo install sniplab
```

Or download a binary from the [latest release](https://github.com/gitkeniwo/snip/releases/latest):

| Platform | Asset |
|---|---|
| macOS (Apple Silicon) | `snip-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `snip-x86_64-apple-darwin.tar.gz` |
| Linux x86_64 | `snip-x86_64-unknown-linux-gnu.tar.gz`, `.deb`, `.rpm` |
| Linux arm64 | `snip-aarch64-unknown-linux-gnu.tar.gz`, `.deb`, `.rpm` |
| Windows x86_64 | `snip-x86_64-pc-windows-msvc.zip` |

```bash
curl -L https://github.com/gitkeniwo/snip/releases/latest/download/snip-aarch64-apple-darwin.tar.gz | tar xz
install -m 755 snip /usr/local/bin/snip
```

### Linux packages

Each release also provides native Linux package files. These are local packages,
not an apt or dnf repository: download the newer release file again when you
want to upgrade.

The current Linux binaries are built on Ubuntu 24.04. The `.deb` packages are
intended for Ubuntu 24.04 and newer, Debian 13 and newer, and their derivatives
(including Linux Mint 22, Pop!_OS 24.04, elementary OS 8, and Zorin OS 17).
Choose the command matching your CPU:

```bash
# Debian / Ubuntu family, x86_64
curl -fLO https://github.com/gitkeniwo/snip/releases/latest/download/snip-x86_64-unknown-linux-gnu.deb && sudo apt install ./snip-x86_64-unknown-linux-gnu.deb

# Debian / Ubuntu family, arm64
curl -fLO https://github.com/gitkeniwo/snip/releases/latest/download/snip-aarch64-unknown-linux-gnu.deb && sudo apt install ./snip-aarch64-unknown-linux-gnu.deb
```

The `.rpm` packages are intended for Fedora 40 and newer. They are available for
both x86_64 and arm64:

```bash
# Fedora, x86_64
curl -fLO https://github.com/gitkeniwo/snip/releases/latest/download/snip-x86_64-unknown-linux-gnu.rpm && sudo dnf install ./snip-x86_64-unknown-linux-gnu.rpm

# Fedora, arm64
curl -fLO https://github.com/gitkeniwo/snip/releases/latest/download/snip-aarch64-unknown-linux-gnu.rpm && sudo dnf install ./snip-aarch64-unknown-linux-gnu.rpm
```

RHEL, Rocky Linux, AlmaLinux, CentOS Stream, Ubuntu 22.04, Debian 12, and
openSUSE Leap use older system libraries than the current builds. Build from
source on those systems for now (for example, `cargo install sniplab`).

### Arch Linux and derivatives

After the first AUR publication, install `snip` with an AUR helper:

```bash
yay -S snip
# or: paru -S snip
```

The AUR package builds from the release source on your machine, rather than
repackaging an Ubuntu-built binary.

Or build from source (Rust 1.89 or newer):

```bash
git clone https://github.com/gitkeniwo/snip.git
cd snip
cargo install --path .
```

For a smaller binary without the terminal browser, build with
`--no-default-features`.

Release history is in [CHANGELOG.md](CHANGELOG.md).

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
snip list --sort modified          # modified | created | title
snip search hello
snip preview Hello
snip edit Hello
snip open Hello                    # hand a managed path to an app
```

Then open the terminal browser:

```bash
snip tui                              # or plain `snip` in an interactive terminal
snip --library ./Main.sniplib tui
```

When `--library` is omitted, `SNIP_LIBRARY` is checked next, then the nearest
`snip.toml` walking up from the current directory, and finally `default_library`
in the user config. Commands run inside a library never jump to the global
default.

## What it does

- **Plain-file library.** One directory per snippet, holding fragments, notes, a
  README, and a TOML manifest. [FORMAT.md](FORMAT.md) specifies it normatively.
- **CLI for everything.** Create, list, search, show, edit, move, tag, trash,
  restore, import, and repair, with `--output json` or `jsonl` on every command.
- **Terminal browser.** Three-pane TUI with syntax highlighting, live reload,
  and mouse support.
- **Concurrency-safe writes.** Library lock, atomic writes, and `--if-hash`
  fingerprint checks so a writer cannot overwrite a version it never read.
- **Structure-aware search.** Regex, field filters, and context lines, so it
  replaces `grep`/`rg` over the library.
- **SnippetsLab import.** Preserves UUIDs, hierarchy, tags, flags, timestamps,
  content, and notes.
- **Optional Git backup.** Commit, push, and fetch scoped to the library, with
  an interactive console in the TUI.
- **Agent skill.** [`skills/snip`](skills/snip/SKILL.md) packages the CLI
  contract for coding agents.

## Terminal browser

Three panes: folders and tags on the left, the snippet list in the middle, a
preview on the right. Selecting a folder or tag filters the list as you move. A
file watcher picks up changes made elsewhere while you are looking at them.

| Key | |
|---|---|
| `:` / `Ctrl-P` | command palette |
| `/` | search |
| `Tab` / `Shift-Tab` | next / previous pane |
| `h`/`←`, `l`/`→` | back out, or drill in |
| `j`/`k` | move; `g`/`G` first/last; `Ctrl-d`/`Ctrl-u` page |
| `1`-`9`, `0` | jump to 1st-10th item (folder, snippet, or fragment) |
| `[`/`]`, `{`/`}` | switch fragments; jump paragraphs |
| `n` | create a snippet, or a folder from the sidebar |
| `e`, `E`, `R` | edit content, note, or README in `$EDITOR` |
| `v` | open in VS Code (`snip open`) |
| `r` | rename a snippet, or the selected folder or tag |
| `m` | move a snippet to a folder, or reparent the selected folder |
| `t`, `f`, `P`, `L` | edit tags, edit current fragment language, toggle pin, toggle lock |
| `y`, `Y`, `p` | copy content (`y`), snippet ID (`Y`), managed path (`p`) |
| `d`, `T` | move to trash; open the restore/purge view |
| `s`, `N`, `z` | change sort order; toggle line numbers; toggle row density |
| `Ctrl-g` | Git console |
| `F5`, `Ctrl-r` | rescan now (the watcher usually does this for you) |
| `?` | the full key map |

Keys are named after the CLI commands they run, so `r` on a folder is
`snip folder rename` and `m` is `snip folder move`.

### Command palette

`:` or `Ctrl-P` opens a palette over whatever you are looking at. Type to
fuzzy-match, `↑`/`↓` (or `Ctrl-p`/`Ctrl-n`) to move, `Enter` to run, `Esc` to
close. It also opens over the help overlay, the trash view, and the Git console.

Keys act on what is in front of you: `r` renames the folder under the sidebar
cursor, or the selected snippet, depending on which pane has focus. The palette
is the other half — every command is spelled out (`Folder: Rename` and
`Snippet: Rename` are separate entries) and runs regardless of focus. Git is
grouped the same way: `Git: Commit`, `Git: Push to Remote`, `Git: Commit with
Message…`.

Commands that cannot run right now stay in the list, greyed out with the reason
on the right (`no snippet selected`, `not a Git repository`) rather than
disappearing. Everything else shows its key binding there instead, so the
palette doubles as a way to pick up the shortcuts.

The mouse works too: click to focus and select, double-click to drill in, click
a fragment tab, and scroll the pane under the cursor. Dragging across the
preview selects text and releasing copies it, without the line-number gutter.

The create wizard's language step is a searchable picker with 74 built-in
languages backed by 220 syntax definitions. Canonical names, common aliases, and
extensions all match (`ts` finds TypeScript, `yml` finds YAML), and a language
not in the list can still be entered and used as-is.

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

## Configuration

The config lives at `$XDG_CONFIG_HOME/snip/config.toml`, or
`~/.config/snip/config.toml` when `XDG_CONFIG_HOME` is unset.

```bash
snip config init --library /path/to/Main.sniplib
snip config show
snip config path
```

Supported values can be changed without editing TOML by hand:

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
snip config set tui-density compact
snip config set tui-line-numbers false
snip config set git-auto-commit-interval 15
snip config set git-auto-push true
snip config set git-backup-on-quit true
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
sort = "modified"          # modified | created | title
density = "comfortable"    # comfortable | compact
line_numbers = true        # preview gutter; toggled with `N`

[git]
auto_commit_interval = 0   # minutes; 0 disables automatic Git operations
auto_push = false          # push ahead commits in the background
backup_on_quit = false
```

`SNIP_TUI_THEME=light|dark` overrides `[tui].theme`. Config values are defaults
only; explicit CLI options override them. Unknown TOML fields are preserved when
`snip config set` or `unset` rewrites the file, so future settings can coexist.

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
can read and write a library without going through snip.

Direct editor changes are discovered on the next scan. CLI mutations use a
library lock and atomic writes. `snip doctor --repair` recovers interrupted
package transactions, and `snip organize` normalizes package directory names.

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
original fingerprint before committing the result. Additional editor targets are
available with `--fragment`, `--note-editor`, `--readme-editor`, and
`--metadata-editor`.

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

## Git backup and deletion

Git is optional. `snip git status` reports the branch, upstream, ahead/behind
counts, uncommitted changes, conflicts, and last commit for the library. File
counts and commits are scoped to the library when it lives inside a larger
repository.

```bash
snip init Main.sniplib --git
snip --library Main.sniplib git init
snip --library Main.sniplib git commit
snip --library Main.sniplib git commit -m "before refactoring"
snip --library Main.sniplib git backup
snip --library Main.sniplib git push
snip --library Main.sniplib git fetch
```

`snip init --git` only applies at creation time; `snip git init` makes an
existing library a repository, and is idempotent when it already is one.
`commit` stages and commits only library content. `backup` commits when the
library is dirty and pushes whenever the branch is ahead of its upstream, so it
also handles a clean worktree with earlier local commits. `push` retries only
the push step. `fetch` refreshes and prunes remote-tracking refs without
changing the worktree. `backup` is idempotent. These CLI operations are
non-interactive and fail rather than waiting for credentials.

In the TUI, `Ctrl-g` opens the Git console: `b` backs up, `c` commits, `p`
pushes, `f` fetches remote status in the background, and `C` enters a custom
message. Automation is editable there too: `i` sets the commit interval, `u`
toggles automatic push, `o` toggles backup on quit, and `a` pauses automation
for the current session. In a library that is not yet a repository, `i`
initializes it.

Automatic behavior is off by default. Set `git-auto-commit-interval` to make the
TUI create a local commit when the library is dirty and the last commit is at
least that many minutes old. Set `git-auto-push` to push ahead commits in a
background worker on the same interval. Automatic work skips conflicts, detached
HEADs, in-progress Git operations, open modals, and a library lock held by
another snip process.

snip never pulls, switches branches, or resolves conflicts. If a push is
rejected, pull and resolve it in your terminal.

`snip delete` moves packages into tracked `trash/`. `snip restore` moves them
back. Permanent deletion requires `snip purge SELECTOR --yes`.

## Shell completion

```bash
snip completion zsh > ~/.zfunc/_snip
snip completion fish > ~/.config/fish/completions/snip.fish
```

Bash, Elvish, and PowerShell are also supported.

## Development

Rust 1.89 or newer. Dependencies are pinned in `Cargo.lock`.

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

`Main.sniplib/` is local working data and is ignored by the repository. It can
be deleted and recreated with `snip init ./Main.sniplib --name Main` without
affecting the Rust project.

### CI

- `CI` runs on pushes and pull requests: formatting, Clippy on both feature
  sets, and tests on Linux (stable, 1.89 MSRV, and `--no-default-features`),
  macOS arm64, and Windows.
- `Deep tests` is manually dispatched for the complete deterministic suite, the
  synthetic SnippetsLab importer fixture, the recursive-watcher regression, and
  an LCOV coverage report.
- `Release build` runs for `v*` tags and manual dispatch, producing archives for
  Linux x86_64/arm64, macOS arm64/Intel, and Windows x86_64. On a tag it also
  bumps the formula in
  [gitkeniwo/homebrew-snip](https://github.com/gitkeniwo/homebrew-snip), which
  needs a `HOMEBREW_TAP_TOKEN` secret with write access to that repository, and
  publishes the crate to crates.io, which needs a `CARGO_REGISTRY_TOKEN` secret.
  Either step is skipped when its secret is absent.

### Releasing

The tag is the single source of truth for the version, so the release job first
checks that `v<tag>` matches the `version` field in `Cargo.toml` and fails the
run if they disagree.

1. Bump `version` in `Cargo.toml` and add the release to
   [CHANGELOG.md](CHANGELOG.md).
2. Commit, then tag the commit `vX.Y.Z` and push the tag.
3. `Release build` builds every platform, attaches the archives to a GitHub
   release, publishes `sniplab` to crates.io, and updates the Homebrew formula.

Publishing to crates.io is idempotent: if that version is already on the
registry the step reports it and skips, so a workflow re-run is safe.

## License

MIT. See [LICENSE](LICENSE).
