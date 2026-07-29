# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.2.1]: https://github.com/gitkeniwo/snip/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/gitkeniwo/snip/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/gitkeniwo/snip/releases/tag/v0.1.0
