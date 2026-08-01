# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.4.0]: https://github.com/gitkeniwo/snip/compare/v0.3.2...v0.4.0
[0.3.1]: https://github.com/gitkeniwo/snip/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/gitkeniwo/snip/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/gitkeniwo/snip/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/gitkeniwo/snip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/gitkeniwo/snip/releases/tag/v0.1.0
