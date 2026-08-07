# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **The TUI keyboard is one table.** Every key now resolves through a keymap of
  *(mode, chord) → action* instead of six hand-written dispatch functions, which
  is the groundwork for user-defined bindings. Four keys behave differently:
  `q` in the trash and in the help pane closes the overlay instead of quitting
  the application (matching the Git console, and `ctrl-c` still quits from
  anywhere); `Home` and `End` jump to the first and last item in every pane, not
  only in the trash; `Ctrl-d` and `Ctrl-u` page the trash as they already paged
  the help pane; and `Alt-s` no longer falls through to `s`, so it stops cycling
  the sort order. Everything else is unchanged.

## [0.5.5] - 2026-08-07

### Changed

- **The README is a first-class preview item.** Snippet-level prose used to be
  appended below whichever fragment was on screen, under a `── readme ──`
  rule — it read as a tail of that fragment and repeated on every switch.
  It is now the last row of the fragment tree, labelled `README.md`, selected
  and previewed like a fragment. `[` from the first fragment wraps straight to
  it, copy and edit follow the selection, and the row appears only when a
  README exists. It remains a non-fragment on disk: no manifest entry, no id,
  and no change to the fingerprint or to `snip fragment`.

## [0.5.4] - 2026-08-07

### Added

- **Standalone install script.** `curl -fsSL
  https://github.com/gitkeniwo/snip/releases/latest/download/install.sh | sh`
  installs `snip` to `~/.local/bin` without `sudo`, on macOS and Linux. Re-run
  it to upgrade and pass `--uninstall` to remove the binary. It verifies the
  download against the published checksums, refuses to downgrade, and leaves an
  installation owned by Homebrew, apt, dnf, nix, or cargo untouched. Manual
  pages and shell completions stay opt-in through `snip man install` and
  `snip completion`.
- **Static musl Linux binaries.** `snip-x86_64-unknown-linux-musl.tar.gz` and
  `snip-aarch64-unknown-linux-musl.tar.gz` are statically linked and have no
  glibc version floor, so they run on Alpine and on distributions older than the
  GNU archives support. The install script uses them on Linux, and
  `cargo binstall sniplab` picks them up on musl hosts.
- **Published checksums.** Every release now carries a `SHA256SUMS` file
  covering all of its assets, plus a detached OpenPGP signature in
  `SHA256SUMS.asc`.

### Changed

- **Lower glibc requirement.** The GNU Linux archives and the `.deb` / `.rpm`
  packages are built on Ubuntu 22.04 and need glibc 2.35 rather than 2.39, so
  Ubuntu 22.04, Debian 12, and their derivatives are supported again.

## [0.5.3] - 2026-08-04

### Added

- `snip git pull [--ff-only]` fetches and merges the configured upstream,
  aborting conflicts without leaving merge markers in the library.
- `git-auto-pull` optionally pulls once in the background when the TUI starts;
  `Ctrl-g l` pulls manually and `Ctrl-g U` toggles the startup behavior.

## [0.5.2] - 2026-08-03

### Added

- **One-command library restore.** `snip git clone` restores a library through
  non-interactive `git clone` or `gh repo clone`, validates it, and can record
  it as the default with `--set-default`.

### Changed

- **Cached preview rendering.** Wrapped preview layouts and styled span runs are reused to reduce repeated allocation and rendering work.

### Fixed

- **Grapheme-safe preview layout.** Preview measurement, truncation, and wrapping now keep Unicode grapheme clusters intact.
- **Cloned libraries open without manual repair.** Missing `snippets/`,
  `trash/`, and `.snip/` runtime directories are recreated automatically, so a
  clone no longer fails with `library is missing directory`.

## [0.5.1] - 2026-08-03

### Added

- **Fragment editing in the terminal browser.** In-place fragment operations in the preview pane: add (`n.add`), rename (`n.rename`), reorder (`n.reorder`), and remove (`n.remove`), matching the CLI's `snip fragment` commands.
- **Fragment grab interaction.** Interactive grab-and-move for reordering fragments directly within a snippet in the TUI.

### Fixed

- **Viewport offset clamping on list refresh and sidebar rebuild.** `refresh_visible` no longer selects an out-of-bounds index on short or filtered lists and clamps list viewport offsets to prevent the cursor row from going off-screen while preserving valid offsets during background rescans. `rebuild_sidebar` clamps excess sidebar offsets similarly.

## [0.5.0] - 2026-08-02

### Added

- **Publish a snippet as a gist.** `snip gist` adds `push`, `url`, `status`,
  `attach`, `detach`, `delete`, and `open`, each with exact human output, a
  stable JSON contract, and `--copy` clipboard support. `push` maps a snippet
  to its publishable `(filename, content)` set, creates or updates the gist,
  and skips the network entirely when a BLAKE3 digest shows nothing changed;
  `open` delegates to `gh gist view --web` so the browser launches on every
  platform.
- **Published copies are recorded in the manifest.** Snippet `snippet.toml`
  files gain a `remotes` table remembering where each copy was published —
  host, id, URL, visibility, published files, and the payload digest. `status`
  recomputes the digest with the recorded payload options, so a push with
  `--include-notes` no longer reports a forever-modified snippet.
- **GitHub CLI runs through a hardened process layer.** `gh` is spawned quiet
  and non-interactive with an `SNIP_GH_BIN` override, and its failures are
  classified — missing binary, missing login, or failed request — with the
  HTTP status pulled from stderr so token-scope problems get precise errors
  and hints.
- **Gist panel in the terminal browser.** `Ctrl-S` opens the gist panel for
  the selected snippet, `P` publishes a public gist, and the list and preview
  draw a gist badge. A published filter in the sidebar toggles to show only
  snippets that have a remote.
- **Regrouped sidebar and trash panes.** The sidebar reads as three groups —
  scopes, filters, then the folder and tag trees — and navigation no longer
  fires row actions: the published toggle waits for Enter or a click instead
  of toggling as the cursor passes over it. The trash view opens in the list
  and preview panes instead of covering them with a popup.
- The preview metadata line now shows created and modified dates, and marker
  separators appear only between markers that are present.
- Confirm modals repeat their confirm and cancel keys inside the box, so a
  modal centred on a tall terminal can never appear with no visible way out.

### Changed

- The README gains shields badges for releases and the install channels, with
  the crates.io downloads badge pinned to the current version.
- The agent skill documents gist publishing, including that `gh` needs the
  `gist` scope and that publishing is public and irreversible.

### Fixed

- Compact TUI rows reserved space for the gist badge but never drew it,
  leaving a phantom gap; they now render it ahead of the date with the date
  column kept aligned.
- An unconditional marker separator left an orphan dot on snippets that were
  neither pinned nor locked; separators are now emitted only between present
  markers.

## [0.4.1] - 2026-08-01

### Added

- **Base16 theme import and a wider built-in set.** `snip theme import` turns
  local base16/base24 scheme files (or stdin) into validated, editable themes,
  and the curated built-ins now cover 28 light and dark palettes.
- **`sniplab-bin` AUR package.** Arch users can install the release binary
  without compiling (`yay -S sniplab-bin`). The release pipeline now maintains
  both the source `sniplab` and prebuilt `sniplab-bin` PKGBUILDs; the two
  conflict so only one is installed at a time.
- **Fedora Copr builds.** Every release is submitted as a source-built SRPM to
  the `gitkeniwo/snip` Copr project, so Fedora and Enterprise Linux 10 users can
  `dnf copr enable gitkeniwo/snip && dnf install sniplab` and let `dnf upgrade`
  pick up new releases. Dependencies are vendored into the SRPM because Copr
  builds run in mock without network access; the step is skipped when
  `COPR_API_CONFIG` is unset.

### Changed

- The portable Unix release archives now also carry `LICENSE` and `README.md`,
  so a packaged install can install the license text and docs.

### Fixed

- Theme validation warnings no longer pin a message over the bottom bar
  indefinitely. Warnings were re-emitted on every auto tick and surfaced on
  startup; they are now shown only when the theme actually changes, and the
  `computed-foreground` finding — an automatic black/white fallback that is
  never actionable at runtime — is downgraded to a Note-level result that
  cannot pin a warning at all.

## [0.4.0] - 2026-08-01

### Added

- **Expandable fragment tree.** A snippet with several fragments renders as
  a selectable tree in the preview instead of a flat tab strip: `=` expands
  it, `-` collapses it, clicking a row jumps to that fragment, and the tree
  stays reachable from the command palette as a registered command.
- **Readable text on every surface.** Foregrounds that miss the WCAG 4.5
  body-text floor against their actual background now fall back, at render
  time, to the most legible colour the theme has. This covers the bottom-bar
  shortcut pills, the top-bar context pill, position pill and breadcrumbs,
  the retained selection in an unfocused pane, and selected fragment-tree
  rows — several built-ins previously drew such text at 1.0:1.

### Changed

- **Generated themes clear stricter contrast floors.** `muted`, `bar_fg`,
  `tag`, `accent`, `accent_alt`, `warning`, `error`, and `success` now blend
  toward black or white until they reach 4.5:1 on their background, `rule`
  needs 3.0:1, and `border` 2.5:1. All fifteen base16 built-ins were
  regenerated.
- **`snip theme check` verifies the pairs the UI actually renders.**
  `foreground-contrast` and `selection-contrast` stay 4.5:1 failures;
  `role-legibility` is now a 4.5:1 warning; `graphic-legibility` covers
  `rule` and `border`; and `computed-foreground` warns when a theme's
  surfaces would force the black/white fallback.

## [0.3.2] - 2026-07-31

### Added

- **Selectable TUI color themes.** Seventeen built-in light and dark themes,
  editable themes under the user config directory, `snip theme` inspection and
  switching commands, `tui-light-theme` / `tui-dark-theme` config keys, and a
  command-palette picker with live full-UI and syntax-highlighting preview.

## [0.3.1] - 2026-07-31

### Added

- Interactive first-run setup for `snip` and `snip init`, plus actionable hints
  when a library cannot be resolved.
- **Nix flake support.** NixOS users can install `pkgs.sniplab` through the
  supplied overlay, other Nix users can use `nix profile install`, and
  `nix run github:gitkeniwo/snip` runs the `snip` binary. The package includes
  shell completions and all manual pages, which are available without
  `snip man install`. A nixpkgs-ready package definition is included under
  `nix/package.nix`.

### Fixed

- The terminal browser's shortcut pills now use the primary color when the
  trash view is active, matching the rest of its visual state.

### Security

- Release source archives for the AUR are now detached-signed and the PKGBUILD
  verifies that signature before building.

## [0.3.0] - 2026-07-30

### Added

- **Manual pages.** One section 1 page per command, generated from the clap
  command tree and committed under `man/`. The Homebrew, `.deb`, `.rpm`, and AUR
  packages install them, so `man snip` and `man snip-create` work after any
  packaged install. CI fails when the checked-in pages drift from the CLI and
  lints their roff, so the pages cannot quietly fall behind `--help`.
- **`snip man`**, for the installations no package manager owns — `cargo install`
  or a downloaded archive. The pages are embedded in the binary, so nothing is
  regenerated on your machine: `snip man path`, `snip man install`,
  `snip man uninstall`, `snip man show [PAGE]`, and `snip man generate DIR`.
  Installs are user-level (`$XDG_DATA_HOME/man/man1`, otherwise
  `~/.local/share/man/man1`) unless `--prefix` says otherwise, and snip never
  invokes `sudo` itself. Installing warns when the destination is missing from
  your `MANPATH`, which is the usual reason a freshly installed page cannot be
  found.
- `snip man install` refuses to overwrite a page it did not write, and
  `snip man uninstall` removes only files whose contents still match its own
  manifest. A page you edited by hand, or one owned by a package manager, is
  reported and left in place.
- **Debian/Ubuntu and Fedora/Enterprise Linux packages.** Every release now
  attaches `.deb` and `.rpm` artifacts next to the portable archives.
- **AUR package** `sniplab`, which builds from the release source rather than
  repackaging the Ubuntu-built binary.
- **Scoop bucket** for Windows: add `gitkeniwo/scoop-snip`, then
  `scoop install snip`.
- `cargo binstall sniplab` metadata, so the prebuilt binary can be installed
  without compiling it.

### Changed

- The Unix release archives now contain `snip` plus a `man/` directory; they
  held only the executable before. The executable is still at the archive root,
  so `cargo binstall` and the Homebrew formula are unaffected, but a script that
  unpacks an archive into a shared directory now receives `man/` as well.

## [0.2.1] - 2026-07-29

### Added

- `snip git init` makes an existing library a Git repository. Previously only
  `snip init --git` could do it, at creation time, so a library created without
  that flag had no supported way back — while the terminal browser could do it
  from the Git console. Idempotent: it reports `created: false` and succeeds on
  a library that is already a repository, and creates no commit.
- `tui-line-numbers` config key. The preview gutter was the one display
  preference that could not be persisted; toggling it with `N` now saves, the
  same way density does. Defaults to on, and configs written before the key
  existed keep that behaviour.

### Fixed

- `App::new` no longer reads the on-disk `state.toml`. Loading it in a
  constructor meant every test inherited whatever the developer's own recent
  commands happened to be, so two palette tests passed or failed depending on
  the machine. The real entry point loads it explicitly instead.
- The help overlay documents `i` in the Git console as initializing a
  repository as well as setting the automatic interval — the key does both
  depending on whether the library is a repo, but only one meaning was listed.

### Documentation

- The agent skill's main page covers `snip git`, `snip info`, and `snip init`,
  and says which commands it deliberately leaves to the reference.
- Corrected the claim that the TUI suspends the terminal for manual Git
  operations; that changed in 0.2.0.

## [0.2.0] - 2026-07-29

First release published to [crates.io](https://crates.io/crates/sniplab). The
crate is named `sniplab`; the binary it installs is still `snip`.

### Added

- **Command palette.** Press `:` or `Ctrl-P` in the terminal browser to search
  every command by name, keyword, or category and run it from one list. Every
  keybinding is now a registered command, so keys and the palette share a single
  execution path.
- Git commands are grouped as palette entries (`Git: Commit`, `Git: Push`,
  `Git: Pull`, …) alongside the existing `Ctrl-G` console, which still works.
- Commands that cannot run right now stay visible and explain why when selected
  (for example `not a Git repository`, `git not found in PATH`) instead of
  silently doing nothing.
- Recently used commands are ranked first, and that history is persisted to
  `~/.config/snip/state.toml` (up to 20 entries) so the ordering survives a
  restart. A missing, corrupt, or newer-schema state file degrades silently.
- The palette shows a position indicator when matches exceed the visible rows.
- `cargo install sniplab` as an installation route.

### Changed

- Manual Git actions run on a background thread and no longer suspend the
  terminal, so the UI stays responsive during a slow push or pull.
- Picker modals (language, folder) rank with `nucleo-matcher` using an explicit
  tier order — exact alias/label match, then exact file-extension match, then
  prefix, then fuzzy — and cache their filtered results. Picker results now
  resolve languages consistently with the CLI.
- The terminal browser's input routing, git panel, preview panel, and modal
  components were each split into focused submodules.

### Fixed

- Language pickers no longer resolve short aliases to the wrong language
  (`java`, `rs`, `c`, `cs`, `r`, `sh`, `m`, `v` all now match what the CLI
  resolves them to).
- The palette no longer overdraws its border or the bottom bar on short
  terminals; visible rows are measured from the popup's actual inner frame.
- Changing the palette query resets the selection, so Enter can no longer run a
  command left over from a previous query.

## [0.1.0] - 2026-07-28

Initial release, distributed as prebuilt binaries and through the
`gitkeniwo/snip` Homebrew tap.

### Added

- **Filesystem-native library.** Snippets, fragments, folders, tags, and
  metadata are stored as ordinary text files that can be grepped, diffed,
  edited by any tool, and tracked in Git. The layout is documented in
  `FORMAT.md`.
- **CLI.** Create, read, update, move, and delete snippets and fragments;
  manage folders and tags; query and list; run `doctor`; generate shell
  completions. Every command speaks JSON, snippets are addressed by UUID, and
  writes are guarded by a content fingerprint so concurrent writers cannot
  clobber each other.
- **Terminal browser.** Sidebar, snippet list, and syntax-highlighted preview,
  with modals for creating and editing, text selection with drag-to-copy,
  digit-key jumps, paragraph navigation, a help overlay, a trash view, and
  configurable themes and compact rows.
- **Git integration.** Status and log parsing, an interactive sync console
  (`Ctrl-G`), auto-commit on an interval, and auto-push.
- **Search.** Regex support, context lines, and field filters.
- **SnippetsLab import**, reading the SnippetsLab library format directly.
- Editor and VS Code handoff from the browser, and clipboard copy of a
  snippet's content or its managed path.
- An agent skill under `skills/snip/` describing the CLI and data model.
- CI, deep-test, and release-build workflows covering Linux, macOS, and Windows.

[0.5.4]: https://github.com/gitkeniwo/snip/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/gitkeniwo/snip/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/gitkeniwo/snip/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/gitkeniwo/snip/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/gitkeniwo/snip/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/gitkeniwo/snip/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/gitkeniwo/snip/compare/v0.3.2...v0.4.0
[0.3.1]: https://github.com/gitkeniwo/snip/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/gitkeniwo/snip/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/gitkeniwo/snip/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/gitkeniwo/snip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/gitkeniwo/snip/releases/tag/v0.1.0
