# Roadmap

Planned work, roughly in the order it is likely to land. Nothing here is a
commitment to a date.

## TUI

### Color themes

- [x] Ship selectable TUI color themes: seventeen built-in light and dark
  themes, editable TOML themes under the user config directory, `snip theme`
  inspection and switching commands, `tui-light-theme` / `tui-dark-theme`
  config keys, and a command-palette picker with live full-UI and
  syntax-highlighting preview. (landed in 0.3.2)

### Theme readability (in progress)

Tracks `docs/plans/theme-contrast.md`. Fix the systemic "unreadable text"
problem (several built-in themes render text at 1.0:1 contrast), then make
themes cheap to extend.

- [x] A — runtime `legible_on` fallback: every pill / retained-selection
  foreground picks a readable color at render time instead of trusting the
  role mapping.
- [x] B — raise the generator's contrast floors to WCAG 4.5 and cover
  `accent` / `accent_alt`; add graphic floors for `rule` (3.0) and `border`
  (2.5); regenerate the built-ins.
- [x] C — rewrite `theme check`'s pairs to the real render pairs and add
  regression tests that no built-in fails.

### Theme import & extension

- [x] Shipped `snip theme import <scheme.yaml>`: the generator's base16 →
  UI-role mapping, contrast clamping, and selection-foreground choice now live
  in a shared library, so local base16/base24 schemes import as editable user
  themes without adding a runtime dependency.
- [x] Curated 28 built-in base16 themes through `SPECS`, including twelve light
  themes, while retaining generated assets and hand-verifiable provenance.

### User-defined key bindings

- [ ] Make every TUI action rebindable through the config file, with the
  current bindings as the default set. Needs a stable action name for each
  command and a way to show the effective bindings (both in the help pane and
  as JSON).

### Grapheme-cluster-aware text width

`char_width` (`src/tui/selection.rs`) measures width per `char` (Unicode
scalar value) and falls back to `.max(1)` for anything ratatui reports as
zero-width. That is correct for CJK and other genuinely wide code points, but
wrong for multi-scalar grapheme clusters: a ZWJ sequence like `👨‍💻` is three
scalars (`👨`, ZWJ, `💻`) that render as one glyph, so `char_width` sums them
to more cells than the terminal actually uses, and `truncate_end`
(`src/tui/widgets.rs`) can cut the string apart right after the ZWJ, leaving a
dangling joiner plus an orphaned trailing emoji. Every truncation and
flush-right layout in the TUI (preview header, snippet list, panel/help text,
fragment tree) is affected, since they all route through `char_width` /
`text_width` / `truncate_end`.

- [x] Switch `char_width`/`truncate_end` to walk grapheme clusters (e.g. via
  `unicode-segmentation`) instead of `chars()`, so multi-scalar emoji and
  other combining sequences are measured and truncated as one unit.
- [x] `char_width` is also used by the prose/code line-wrapping in
  `src/tui/preview/layout.rs`, so a fix needs to keep that wrapping correct
  too, not just the truncation call sites.
- [x] Add a regression test with a ZWJ emoji sequence in a title, mirroring
  the existing CJK-width tests in `src/tui/widgets.rs` and
  `src/tui/preview/render.rs`.
- [ ] Expand tabs before syntax highlighting using an explicit tab-stop policy;
  ratatui filters the control character itself, so tabs currently collapse to
  one visual column. This is independent of grapheme-cluster measurement.

Found while fixing commit `3558bc2` ("fix(tui): keep flush-right markers and
dates with wide glyphs") and its follow-up, which made truncation cell-aware
for CJK — but the emoji case that commit's message also claimed to handle was
never actually correct.

### Configurable snippet editor

- [ ] Let the user pick per preference:
  - an in-TUI editor (`tui-textarea`), for quick edits without leaving the
    browser
  - `$EDITOR`, for terminal editors
  - an external GUI editor, launched detached
- [ ] The choice belongs in the config file, with a sensible fallback chain
  when the configured editor is missing.

### Fragment editing

The TUI covers the full fragment surface — add, rename, reorder, remove, and
browsing — via the command registry (`src/tui/command/registry.rs`):
`n.add`, `n.rename`, `n.reorder`, `n.remove`, and `view.toggle-fragment-list`.

- [x] Add/create a fragment from the preview's fragment tree.
- [x] Rename a fragment title in place.
- [x] Move a fragment to another position within the snippet.
- [x] Delete a fragment with a confirmation prompt.

### Preview render cost per frame

Measured RSS for `snip tui` under a PTY: ~9.8 MB at 2s, ~11.5 MB at 6s, then
flat at 11.5–11.8 MB through 15s — no growth, no leak. CLI commands sit at
6.6–7.3 MB, so the TUI's increment is ~4.5 MB. An empty library still costs
11.0 MB, and a 59-snippet library 11.8 MB, so library content is only ~0.8 MB;
the rest is fixed (~5.4 MB binary/runtime baseline shared with the CLI, plus
`two_face::syntax::extra_newlines()`, ratatui, and the file watcher). That
total is healthy for a Rust TUI carrying a full syntax set, and is not worth
optimizing on its own.

What the measurement did surface is a per-frame allocation problem in the
preview pane. The rendered-preview cache now removes steps 2–4 on a cache hit;
the remaining per-frame work is step 1, for the selected snippet:

1. deep-copies the whole `Snippet` — including every loaded fragment's full
   text — at `src/tui/preview/render.rs:21` (`selected_snippet().cloned()`);
2. deep-copies the cached `PreviewDocument` on a cache *hit*
   (`src/tui/preview/cache.rs:37`), which is one owned `String` plus a `Style`
   per highlight token;
3. re-runs `compose_preview` (`src/tui/preview/layout.rs:20`), which consumes
   that document by value and rebuilds every line to add gutters and insets;
4. re-runs `wrap_preview` over the whole fragment, not just the visible
   window.

Steps 2–4 all repeat work whose inputs did not change. `jump_paragraph`
(`src/tui/preview/layout.rs:347`) runs the same pipeline again, though only on
a keypress, not per frame. The cost scales with fragment length, so it shows
up as input latency on large snippets.

- [x] Cache the *rendered* preview instead of the intermediate document: key
  on `(fingerprint, fragment_index, content width, show_line_numbers)` and
  reuse the wrapped output across frames. This removes the document clone,
  the recompose, and the rewrap in one change, and makes an
  `Arc<PreviewDocument>` unnecessary.
- [ ] Borrow the selected snippet in `draw_preview` rather than cloning it;
  the clone exists to end the `&App` borrow before `&mut App` is needed, so
  this needs the borrow split untangled first. Deliberately retained after the
  preview-cache work: the normal and trash preview ownership paths need a
  wider borrow refactor, for a much smaller payoff than the removed wrapping
  allocations.
- [x] Add a counted-allocation regression guard for both direct cache hits and
  the full preview draw path, including selection synchronization and per-Line
  rendering.

Two follow-ups were considered and rejected. Making the syntax set lazy buys
nothing: the TUI draws the preview on its very first frame (`src/tui/ui.rs`
calls `draw_preview` unconditionally), so a lazy `Highlighter` would
initialize milliseconds after startup anyway; only a genuinely reduced
`SyntaxSet` would shrink that cost, at the price of dropping language support.
Highlighting only the visible window is likewise not worth it — syntect's
`HighlightLines` is a stateful line scanner, so reaching line N still requires
scanning lines 1..N; and once the render cache above lands, highlighting runs
once per fragment switch rather than per frame.

### Catalog held twice in memory

`App` holds a `CatalogSnapshot` and a `MemoryIndex` that is nothing but a
second full copy of the same snapshot (`src/tui/app/core.rs:51` and `:496`
call `MemoryIndex::new(catalog.clone())`; `src/search.rs:106` shows the struct
has one field and builds no index — `search()` is a linear scan over
`catalog.snippets`). A rescan transiently holds a third copy. At the current
scale this is only the ~0.8 MB measured above, but it grows linearly with
library size.

- [ ] Share one `Arc<CatalogSnapshot>` between `App` and `MemoryIndex`, or
  have the search borrow the catalog directly, so the snapshot is stored once.
- [ ] Separately: `MemoryIndex` is misnamed for what it does. Decide whether
  it should acquire a real index (inverted index over titles/tags, or a
  prefilter) or be renamed to reflect that it is a linear scanner. This
  depends on the library size we intend to support and is not urgent at
  today's scale.

## Library portability

A library backed up with `snip git backup` can now be restored on a second
machine without hand-editing the filesystem. The three completed items below
landed in order: the first is a bug fix, the second removes the remaining manual
steps, and the third gives the backup loop an entry point. Tracks
`docs/plans/library-restore.md`, which fixes the command surface, the exact
output strings, and the error messages.

### Runtime directories must survive a clone

`snip init` writes a `.gitignore` excluding `.snip/cache/`, `.snip/locks/`, and
`.snip/transactions/` (`src/filesystem/library.rs:77`) — which is every
subdirectory `.snip/` has. Git does not record empty directories, so `.snip/`
does not exist in the repository at all, and a clone arrives without it.
`Library::open` previously rejected the library outright
(`src/filesystem/library.rs:125`), and the fix is unreachable from the CLI:
`snip doctor --repair` takes an already-open `Library` (`src/service/doctor.rs:43`,
dispatched at `src/commands/mod.rs:86`), and `snip init` on an existing path
routes to `finish_connected` (`src/commands/onboard.rs:87`), which opens the
library too. Both used to fail with the same error; self-healing in
`Library::open` now covers every entry point.

The same trap applies to `snippets/` and `trash/`: neither is ignored, but an
empty one is equally absent from a clone and equally fatal on open.

- [x] `Library::open` now self-heals: it creates any of `snippets/`, `trash/`,
  `.snip/cache/`, `.snip/locks/`, `.snip/transactions/` that are missing, and
  keeps the existing error only when creation fails. Best-effort rather than
  `?`, since `open` is on read-only paths too and a read-only mount should not
  block reads that need no scratch space.
- [x] Do not commit a tracked `.snip/.gitkeep`: self-healing makes it
  unnecessary and it contradicts "`.snip/` MUST NOT be committed" in
  FORMAT.md.
- [x] Regression tests build a library, delete each recreatable directory, and
  assert `open` succeeds and a write still works, and a `tests/git.rs` case
  clones a committed library into a fresh path and runs a command against it, so
  the real failure mode is covered end to end.
- [x] The implementation matches FORMAT.md's requirement to recreate runtime
  directories on open only after `snip.toml` proves the directory is a library.

### Restoring a library on another machine

The underlying restore remains clone, then `snip config set default-library`,
because `default_library` lives in
`$XDG_CONFIG_HOME/snip/config.toml` and is deliberately not part of the library.
`snip git clone --set-default` provides both steps as one non-interactive
command.

- [x] `NO_LIBRARY_HINT` now names the path for an existing or freshly cloned
  library as well as the create path.
- [x] The onboarding wizard's unchanged connect branch accepts incomplete
  clones through `Library::open` self-healing.
- [x] The restore flow is documented in the generated `snip-git` manual and
  the README.

### `snip git clone`

The backup loop now has its restore entry point alongside `git init`, `commit`,
`backup`, `push`, and `fetch`:

```bash
snip git clone <remote> [path] [--gh] [--set-default]
```

- [x] Added `Clone` to `GitCommand` and dispatch it before
  library resolution, next to `Init` and `Import` (`src/commands/mod.rs:55`) —
  every other Git subcommand receives an open `Library`, and this one runs when
  no library exists yet.
- [x] The destination defaults to `~/<repo-name>.sniplib`, refuses a non-empty
  target, and validates after cloning that the result really is a library
  (`snip.toml` present, schema accepted) — deleting a clone that is not one, so
  a typo'd remote does not leave a stray directory.
- [x] Credentials remain delegated explicitly: Git is the default and `--gh`
  selects `gh repo clone`; both paths are non-interactive and fail with a
  targeted hint instead of prompting.
- [x] `--set-default` updates the config without prompting; otherwise the
  exact `snip config set default-library` command is printed.
- [x] Shipped `man/snip-git-clone.1` with the other per-subcommand pages, and
  added the command to `man/snip-git.1` and the README.
- [x] Dropped the manual `mkdir` note from the README's "Restoring
  a library on another machine" and pointed the first-run wizard at it.

## Sharing

### Publish a snippet as a Gist

- [x] `snip gist` publishes a snippet to GitHub Gists through `gh`, with no
  embedded API client and no token handling. Fragments become gist files, the
  README becomes `README.md`, and the gist is recorded in `snippet.toml` so
  `push` keeps the same URL. `push`, `url`, `status`, `attach`, `detach`,
  `delete`, and `open` are the command set.
- [ ] GitLab snippets through `glab`, reusing the `[[remotes]]` record with a
  second `kind`.
- [x] TUI integration: a `Ctrl-s` gist panel for the selected snippet, palette
  entries for every gist command, a `Published` sidebar toggle that composes
  with the folder and tag filters, and list/preview markers that appear only for
  published snippets.

## Data import

### SnippetsLab importer audit

`snip import snippetslab` reads the legacy SnippetsLab library bundle directly
(`src/importer/snippetslab/`). It landed in 0.1.0 and has only been refactored
since — never re-validated against a current SnippetsLab export. The code reads
`version.plist` but only for the report; it does not adapt to the format that
version describes, and the tests use a hand-built fixture rather than a real
library.

- [ ] Import a real, current SnippetsLab library (both 1.x and 2.x exports) and
  confirm fields, folders, tags, fragments, and notes still land correctly;
  update the field mappings for any format the current decoder misses.
- [ ] Commit a small, real-world fixture (or a sanitized dump) and a regression
  test that imports it, so future format drift fails CI instead of surfacing as
  a broken user report.

## Project homepage

A lightweight showcase site for snip, kept distinct from the README so the
repo stays the single source of truth.

- [ ] Build a static landing page: tagline, feature highlights, screenshots of
  the TUI, a one-line install command, and links to the README, manual pages,
  and the latest release.
- [ ] Derive the page content from `README.md` (generated or synced), so the
  copy never drifts from the repo.
- [ ] Serve it at a stable, branded URL (planned: `snip.gitkeniwo.tech`) and
  point the README badge links at it.

## Packaging

### nixpkgs submission

- [ ] `nix/package.nix` is written to nixpkgs conventions and is ready to move
  to `pkgs/by-name/sn/sniplab/package.nix`. Before opening the PR:
  - [ ] run `nixfmt-rfc-style` on it, and fix everything `nixpkgs-hammering`
    reports
  - [ ] fill in `meta.maintainers`; a first-time submitter also has to add
    themselves to `maintainers/maintainer-list.nix`, either in the same PR or
    in one before it
- [ ] Refresh the `version` and both hashes in `nix/package.nix` once, right
  before opening the PR (the flake never reads them).
- [ ] Once it is in nixpkgs, users can install `pkgs.sniplab` without adding a
  flake input at all, and the flake in this repository stays for tracking
  `main`.

### Binary cache

- [x] Create a Cachix cache named `snip` and set `CACHIX_AUTH_TOKEN` in the
  repo secrets, so the guarded Cachix steps in the `Nix` and `Release build`
  workflows push prebuilt binaries and `nix run` / `nix profile install`
  download them instead of building from source. (verified working on
  x86_64-linux, aarch64-linux, and aarch64-darwin)

### Arch binary package

- [x] Ship `sniplab-bin` on the AUR so Arch users can
  `yay -S sniplab-bin` without compiling. The release `aur` job now renders
  both `sniplab` (source) and `sniplab-bin` (prebuilt) PKGBUILDs, and the
  portable Linux archives carry `LICENSE` and `README.md` for the package
  to install. The AUR repo is auto-created on the first push.

### Standalone install script

A curl-piped installer for platforms that have no package channel. Official
repositories like apt will not take a new project, and `snip` has no built-in
self-update, so the script also becomes the upgrade path.

- [ ] `install.sh` fetches the release asset for the host platform and
  architecture (`snip-<target>.tar.gz`) and installs the `snip` binary
  user-level (`~/.local/bin`), needing no `sudo`.
- [ ] Resolve the target triple from `uname` / `uname -m` through a small
  table, so new targets (e.g. `armv7`, `riscv64`) slot in by adding a row;
  fall back to `cargo install sniplab` with a clear message when the platform
  has no prebuilt asset (e.g. glibc older than 2.39).
- [ ] Install the man pages from the archive's `man/` into
  `$XDG_DATA_HOME/man/man1` and the shell completions via
  `snip completion bash|zsh|fish`, matching the manual routes the README
  already documents.
- [ ] Idempotent and upgrade-safe: re-running replaces the binary and refreshes
  man pages and completions, prints the old and new versions, refuses to
  downgrade, and leaves an install owned by a package manager
  (brew / apt / dnf / nix) untouched.
- [ ] Ship an `uninstall` mode and keep the script behind a stable URL; the
  release tar.gz layout (`snip`, `man/`, `LICENSE`, `README.md`) is already
  fixed, so no release-workflow change is needed for this to land.
