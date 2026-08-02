# `snip`: a terminal snippet manager

<img width="1132" height="715" alt="image" src="https://github.com/user-attachments/assets/9cffc455-d8c6-4ab4-a63a-9bfc5377f512" />

<p align="center">

[![Crates.io](https://img.shields.io/crates/v/sniplab?color=blue)](https://crates.io/crates/sniplab)
[![Release](https://img.shields.io/github/v/release/gitkeniwo/snip?color=blue)](https://github.com/gitkeniwo/snip/releases/latest)
[![AUR](https://img.shields.io/aur/version/sniplab-bin?color=blue)](https://aur.archlinux.org/packages/sniplab-bin)
[![Downloads](https://img.shields.io/crates/d/sniplab?color=green)](https://crates.io/crates/sniplab)
[![Release downloads](https://img.shields.io/github/downloads/gitkeniwo/snip/total?color=green&label=release%20downloads)](https://github.com/gitkeniwo/snip/releases)
[![Docs.rs](https://img.shields.io/docsrs/sniplab?color=blueviolet)](https://docs.rs/sniplab)
[![MSRV](https://img.shields.io/badge/MSRV-1.89%2B-blueviolet)](https://github.com/gitkeniwo/snip/blob/main/Cargo.toml)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey)](https://github.com/gitkeniwo/snip#install)
[![Homebrew](https://img.shields.io/badge/homebrew-tap-lightgrey)](https://github.com/gitkeniwo/homebrew-snip)
[![Copr](https://img.shields.io/badge/copr-rpm-lightgrey)](https://copr.fedorainfracloud.org/coprs/gitkeniwo/snip/)
[![Scoop](https://img.shields.io/badge/scoop-bucket-lightgrey)](https://github.com/gitkeniwo/scoop-snip)
[![Cachix](https://img.shields.io/badge/cachix-snip-lightgrey)](https://app.cachix.org/cache/snip)
[![CI](https://img.shields.io/github/actions/workflow/status/gitkeniwo/snip/ci.yml?branch=main)](https://github.com/gitkeniwo/snip/actions)

</p>

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

- [Install](#install) · [Quick start](#quick-start) · [What it does](#what-it-does)
- [Terminal browser](#terminal-browser) · [Agent-friendly operations](#agent-friendly-operations)
- [AI agent skills](#ai-agent-skills) · [Configuration](#configuration)
- [Files are the database](#files-are-the-database) · [Preview and editing](#preview-and-editing)
- [SnippetsLab migration](#snippetslab-migration) · [Git backup and deletion](#git-backup-and-deletion)
- [Share as a gist](#share-as-a-gist) · [Manual pages](#manual-pages) · [Shell completion](#shell-completion)
- [Development](#development) · [License](#license)

## Install

The crate is `sniplab`; the binary it installs is `snip`.

### macOS / Linux (Homebrew)

```bash
brew install gitkeniwo/snip/snip
```

### Cargo

Prebuilt binary via [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), no compilation:

```bash
cargo binstall sniplab
```

Or build from crates.io (Rust 1.89 or newer):

```bash
cargo install sniplab
```

### Debian / Ubuntu

For Ubuntu 24.04+, Debian 13+, and derivatives (Mint 22, Pop!_OS 24.04,
elementary OS 8, Zorin OS 17):

```bash
curl -fLO https://github.com/gitkeniwo/snip/releases/latest/download/snip-x86_64-unknown-linux-gnu.deb
sudo apt install ./snip-x86_64-unknown-linux-gnu.deb
```

On arm64, swap `x86_64` for `aarch64`.

### Fedora / Enterprise Linux

From [Copr](https://copr.fedorainfracloud.org/coprs/gitkeniwo/snip/). Builds
from source on Fedora's infrastructure, and `dnf upgrade` picks up new releases:

```bash
sudo dnf copr enable gitkeniwo/snip
sudo dnf install sniplab
```

Or grab the standalone `.rpm` from a release. This works on Fedora 40+ and the
Enterprise Linux 10 family — RHEL 10.0+, Rocky Linux 10.0+, AlmaLinux 10.0+,
Oracle Linux 10+, CentOS Stream 10+ — all of which ship glibc 2.39 or newer:

```bash
curl -fLO https://github.com/gitkeniwo/snip/releases/latest/download/snip-x86_64-unknown-linux-gnu.rpm
sudo dnf install ./snip-x86_64-unknown-linux-gnu.rpm
```

On arm64, swap `x86_64` for `aarch64`.

### Arch Linux

From the AUR. The prebuilt `sniplab-bin` installs the release binary without
compiling:

```bash
yay -S sniplab-bin
```

`paru -S sniplab-bin` works too. If you prefer to build from source, install
`sniplab` instead 

```bash
yay -S sniplab
```

Or clone `https://aur.archlinux.org/sniplab.git` and run
`makepkg -si`. Both packages provide `snip`, so they conflict with each
other.

### Nix

Needs the `nix-command` and `flakes` experimental features. Linux and Apple
silicon; nixpkgs no longer supports Intel macOS.

```bash
nix profile install github:gitkeniwo/snip
```

The first run asks whether to trust the project's Cachix substituter; accept it
to download a prebuilt binary instead of compiling on your machine.

Append a tag to pin a release: `github:gitkeniwo/snip/vX.Y.Z`. Or run it without
installing:

```bash
nix run github:gitkeniwo/snip -- list
```

On NixOS, install it declaratively instead — add the flake input, then apply the
overlay in a module:

```nix
inputs.snip = {
  url = "github:gitkeniwo/snip";
  inputs.nixpkgs.follows = "nixpkgs";
};

nixpkgs.overlays = [ inputs.snip.overlays.default ];
environment.systemPackages = [ pkgs.sniplab ];
```

Manual pages and shell completions ship with the package, so `man snip` works
without `snip man install`.

### Windows (Scoop)

```bash
scoop bucket add snip https://github.com/gitkeniwo/scoop-snip
scoop install snip
```

`scoop update snip` picks up new releases. x86_64 only for now.

### Manual download

Binaries and packages are on the
[latest release](https://github.com/gitkeniwo/snip/releases/latest):

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

The Unix archives also carry `man/`; see [Manual pages](#manual-pages).

### From source

```bash
git clone https://github.com/gitkeniwo/snip.git
cd snip
cargo install --path .
```

For a smaller binary without the terminal browser, add `--no-default-features`.

### Notes

Linux binaries are built on Ubuntu 24.04, so they need glibc 2.39 or newer.
Older systems — RHEL/Rocky/AlmaLinux/Oracle/CentOS Stream 9 and older
(glibc 2.34), Amazon Linux 2023 (2.34), Amazon Linux 2 (2.26), Ubuntu 22.04
(2.35), Debian 12 (2.36), openSUSE Leap 15 (2.31) — fall short; install with
`cargo install sniplab` there. Upgrading a `.deb`/`.rpm` means downloading the newer
file again.

Release history is in [CHANGELOG.md](CHANGELOG.md).

## Quick start

Install `snip`, then run it in an interactive terminal and answer the three
setup questions. It creates (or connects) a library and can make it your
default:

```bash
snip
```

For scripts and remote shells, use the non-interactive path instead:

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

Three panes: the library on the left, snippets in the middle, a preview on the
right. A file watcher picks up changes made outside the browser.

The left pane groups its rows: the scopes you can be in (`All snippets`,
`Uncategorized`, `Trash`), then `Filters`, then the folder and tag trees.
Moving the cursor onto a scope applies it, `Trash` included — it opens in the
snippet pane rather than a popup, so the sidebar stays usable and the preview
shows the deleted snippet before you restore it with `u` or purge it with `x`.
`Published`, under `Filters`, is a toggle rather than a place: it narrows
whatever you are already looking at, and waits for `Enter` or a click so that
merely scrolling past it does nothing.

| Key | |
|---|---|
| `?` | the full key map |
| `:` / `Ctrl-P` | command palette |
| `/` | search |
| `Tab`, `h`/`l` | switch pane; back out or drill in |
| `j`/`k`, `g`/`G` | move; first/last |
| `n`, `e`, `d` | create, edit in `$EDITOR`, move to trash |
| `y` | copy content |
| `Ctrl-g`, `Ctrl-s` | Git console, gist panel |

Keys are named after the CLI commands they run, so `r` on a folder is `snip
folder rename`. The mouse works as expected: click to focus, double-click to
drill in, scroll the pane under the cursor, drag across the preview to copy.

### Command palette

`:` or `Ctrl-P` opens a fuzzy-matched list of every command, each spelled out
(`Folder: Rename`, `Snippet: Rename`, `Git: Commit`) and runnable regardless of
which pane has focus. Commands that cannot run right now stay visible, greyed
out with the reason; the rest show their key binding, so the palette doubles as
a way to learn the shortcuts.

### Appearance

On macOS the TUI follows the system light/dark setting and updates while it
runs; Linux uses `GTK_THEME` or `COLORFGBG` when available. Override it for a
terminal whose background differs from the system:

```bash
SNIP_TUI_THEME=light snip     # or: snip config set tui-theme light
```

Any other `SNIP_TUI_THEME` value selects a theme by name for that run — see
[Themes](#themes).

### Themes

Open the command palette and run **Change Color Theme** to preview every theme
for the current light or dark appearance; `Esc` restores the previous theme and
`Enter` saves the choice. The same slots can be set from the command line:

```bash
snip theme use dark-nord                  # writes to the theme's own slot
snip theme list                           # every installed name
snip theme show dark-nord                 # the resolved colors
```

`snip theme use` saves to the theme's own slot unless `--appearance light|dark`
forces another. Theme *names* go in the `tui-light-theme`/`tui-dark-theme` slots
(see [Configuration](#configuration)); `tui-theme` only picks `auto|light|dark`.

User themes live in the directory printed by `snip theme path` (normally
`~/.config/snip/themes`). See [docs/themes.md](docs/themes.md) for the complete
format, inheritance, validation, and base16 role mapping; `snip theme import
SCHEME.yaml` converts a base16 or base24 scheme into an editable local theme.

Language badges are plain ASCII (`[rs]`, `[py]`, `[sh]`, `[md]`) so they render
in any font. The rounded caps on the top and bottom bars are Powerline glyphs,
which need a Nerd Font or another Powerline-patched terminal font.

## Agent-friendly operations

Every command takes `--output json` or `jsonl`. Use UUIDs from that output for
deterministic operations; titles are accepted only when they identify exactly one
snippet.

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

`search` results carry the snippet's `fingerprint`, so a metadata change can go
straight to `--if-hash` without a separate `show`. Commands that would open an
editor fail with a usage error when stdout is not a terminal, so scripts never
block on an editor that cannot appear.

Structured stdout is kept separate from errors. Exit codes are stable:

| Code | Meaning |
|---:|---|
| 0 | success |
| 1 | I/O or internal failure |
| 2 | invalid CLI usage |
| 3 | missing or ambiguous selector |
| 4 | lock or fingerprint conflict |
| 5 | invalid library data |

## AI agent skills

[`skills/snip`](skills/snip/SKILL.md) packages all of the above into a skill any
agent can load: vocabulary, selectors, JSON payload shapes, and the `--if-hash`
workflow. Symlinking keeps it current as the CLI evolves.

```bash
mkdir -p ~/.agents/skills && ln -s "$PWD/skills/snip" ~/.agents/skills/snip
```

`~/.agents/skills/` is the vendor-neutral location. Support for it is still
uneven, so also link it into the directory your agent actually reads:
`~/.claude/skills/` (Claude Code), `~/.codex/skills/` (Codex),
`~/.opencode/skills/` (opencode), `~/.omp/skills/` (oh-my-pi). One pass covers
whichever of them exist:

```bash
for dir in ~/.agents ~/.claude ~/.codex ~/.opencode ~/.omp; do
  [ -d "$dir" ] || continue
  mkdir -p "$dir/skills"
  ln -sfn "$PWD/skills/snip" "$dir/skills/snip"
done
```

Or hand the job to the agent itself:

```text
Install the snip agent skill from
https://github.com/gitkeniwo/snip/tree/main/skills/snip

Fetch that directory (SKILL.md plus everything under references/) and put it at
~/.agents/skills/snip if you read that location, otherwise your own skills
directory. If the snip repository is already cloned locally, symlink its
skills/snip instead of copying, so the skill tracks the CLI. Then tell me the
path you used and confirm `snip --version` runs.
```

Once loaded, the agent works the library directly:

```text
Save the fish function we just wrote to snip under Shell/Fish, tag it fish and
clipboard, and add a note explaining why the pbcopy fallback is there.

My fish config drifted from the copy in snip. Diff ~/.config/fish/config.fish
against that snippet and update whichever is stale, keeping the note intact.

Run brew bundle dump, compare it with the Brewfile snippet in snip, and commit
the update to the library's Git repo if anything changed.
```

The skill assumes `snip` is on `PATH` and a library is reachable. See
[`skills/README.md`](skills/README.md) for project-scoped installs, the Claude
Agent SDK, and runtimes that take a system prompt instead of skill files.

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
snip config set tui-light-theme light-default
snip config set tui-dark-theme dark-default
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
light_theme = "light-default"
dark_theme = "dark-default"
sort = "modified"          # modified | created | title
density = "comfortable"    # comfortable | compact
line_numbers = true        # preview gutter; toggled with `N`

[git]
auto_commit_interval = 0   # minutes; 0 disables automatic Git operations
auto_push = false          # push ahead commits in the background
backup_on_quit = false
```

`SNIP_TUI_THEME=light|dark` overrides `[tui].theme`; any other non-empty value
selects a theme by name for that run. Config values are defaults
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

The path below `snippets/` *is* the folder hierarchy, and a directory is a
snippet if it contains `snippet.toml`. Directory names are descriptive only, so
you can move or rename them by hand; the UUID in the manifest is the stable
identity. Nothing under `.snip/` is user data, and deleting it while snip is not
running never loses anything.

Editor changes are picked up on the next scan. CLI writes take a library lock
and are atomic, so `snip doctor --repair` can recover an interrupted one.

[FORMAT.md](FORMAT.md) specifies the format normatively, so another tool can
read and write a library without going through snip.

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

## Share as a gist

`snip gist` publishes a snippet to GitHub Gists through the
[GitHub CLI](https://cli.github.com), so snip never handles a token. Install
`gh` and authorize the `gist` scope once:

```bash
gh auth login
gh auth refresh -h github.com -s gist
```

Each fragment becomes one gist file and the README becomes `README.md`, so a
multi-fragment snippet arrives intact. Gists are secret unless you ask for
`--public`.

```bash
snip gist push Brewfile          # publish, and update on later pushes
snip gist url Brewfile --copy    # copy the link
snip gist status Brewfile        # local vs published, no network
snip gist delete Brewfile --yes
```

`push` keeps the same URL for the life of the snippet, so a link you already
shared stays valid, and records the gist in `snippet.toml` so it travels with
the library. It skips the network entirely when nothing has changed. `attach`
adopts a gist you created elsewhere and `detach` forgets one without deleting
it. snip only manages the files it published, so anything you add to the gist
in the browser is left alone.

Visibility is fixed at creation — GitHub cannot turn a secret gist public — so
changing it means publishing a new one with `--new`.

In the TUI, `Ctrl-s` opens the gist panel for the selected snippet: `p`
publishes or updates, `y` copies the link, `o` opens it in a browser, and `a`,
`r`, `d`, `x` link an existing gist, check it still exists, unlink, and delete.
`P` publishes a *public* gist and is offered only before the first publish;
`P`, `d`, and `x` confirm first. Published snippets carry `G✓` in the list and
preview header, or `G+` once you have edited since publishing. Unpublished
snippets carry no marker.

## Manual pages

Homebrew, deb, rpm, and AUR packages install them, so `man snip` works right
away. For `cargo install` or a downloaded archive, install the pages embedded
in the binary:

```bash
snip man path                                # where they will go
snip man install                             # default ~/.local/share/man/man1
sudo snip man install --prefix /usr/local    # system-wide, never implicit
snip man uninstall
```

`snip man install` warns when the destination is missing from your `MANPATH`
and prints the line to add. Uninstalling keeps any page you edited yourself.
`snip man show snip-create` reads a page without installing anything, and
`snip man generate DIR` exports all of them. Windows has no `man`; use
`snip --help`.

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

Man pages are generated from the clap command tree and committed in `man/`.
Update them after changing CLI help, check that they are current, or preview a
page locally:

```bash
cargo run --locked --all-features --example generate-man
cargo run --locked --all-features --example generate-man -- --check
cargo run --locked --all-features --example generate-man -- --preview
cargo run --locked --all-features --example generate-man -- --preview snip-create
```

`Main.sniplib/` is a scratch library for development, ignored by the repository.
Recreate it any time with `snip init ./Main.sniplib --name Main`.

### CI

| Workflow | |
|---|---|
| `CI` | pushes and PRs: fmt, Clippy on both feature sets, tests on Linux (stable, 1.89 MSRV, `--no-default-features`), macOS arm64, and Windows; checks generated man pages and lints their roff |
| `Nix` | pushes and PRs: builds the flake on Linux (x86_64, arm64) and macOS, checks the installed layout, and verifies the hashes in `nix/package.nix` |
| `Deep tests` | manual: full deterministic suite, importer fixture, watcher regression, coverage |
| `Release build` | `v*` tags and manual: builds every platform, then publishes (see below) |

### Releasing

1. Bump `version` in `Cargo.toml`, then rerun
   `cargo run --locked --all-features --example generate-man` — the pages embed
   the version, so CI fails without it.
2. Add the release to [CHANGELOG.md](CHANGELOG.md).
3. Commit, then tag `vX.Y.Z` and push the tag.

The tag is the source of truth: the run fails if it disagrees with `Cargo.toml`.
It then attaches the platform archives and a signed `sniplab-X.Y.Z.tar.gz`
source archive to a GitHub release. The AUR package and the Copr SRPM both build
from that source archive; the AUR package also verifies its detached PGP
signature.

Before the first release, configure the following repository secrets and
variable. `PGP_PRIVATE_KEY` must be an ASCII-armored private signing-key export;
keep it and its passphrase out of the repository. `PGP_FINGERPRINT` is a
repository variable (not a secret) containing the uppercase, whitespace-free
primary-key fingerprint used in the AUR `validpgpkeys` field.

| Configuration | Purpose |
|---|---|
| Secret `PGP_PRIVATE_KEY` | ASCII-armored private key used only to sign release source archives |
| Secret `PGP_PASSPHRASE` | Passphrase for that private key |
| Variable `PGP_FINGERPRINT` | Public primary-key fingerprint expected by the release workflow and AUR |

It then updates each downstream package, every one gated on its own secret and
skipped when that secret is absent:

| Target | Secret |
|---|---|
| crates.io | `CARGO_REGISTRY_TOKEN` |
| [homebrew-snip](https://github.com/gitkeniwo/homebrew-snip) | `HOMEBREW_TAP_TOKEN` |
| AUR (`sniplab`) | `AUR_SSH_PRIVATE_KEY` |
| [scoop-snip](https://github.com/gitkeniwo/scoop-snip) | `SCOOP_BUCKET_TOKEN` |
| [Copr](https://copr.fedorainfracloud.org/coprs/gitkeniwo/snip/) (`sniplab`) | `COPR_API_CONFIG` |

Copr builds run in mock without network access, so the release job vendors the
crate dependencies into the SRPM before submitting it.

The workflow rewrites only URLs and checksums, so the tap's own
`man1.install Dir["man/*.1"]` line is maintained in `gitkeniwo/homebrew-snip`.

Re-running a release is safe: the crates.io step skips a version already on the
registry, and the package updates are no-ops when nothing changed.

## License

MIT. See [LICENSE](LICENSE).
