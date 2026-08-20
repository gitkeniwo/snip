# Roadmap

Planned work, roughly in the order it is likely to land. Nothing here is a
commitment to a date.

## TUI

### Inline preview editor — deferred, not scheduled

The preview stays read-only; editing goes through `e` (`$EDITOR`) and `v` (GUI).
An in-TUI `tui-textarea` editor was designed across four rounds and deliberately
shelved: the cheaper it is, the less a vim user wants it, and the more usable it
becomes, the more it duplicates `e`. The blocker is cursor positioning — `h`/`l`
are globally bound before pane dispatch, so keyboard motion needs mode isolation,
which lands back on emulating vim badly.

Not a to-do. Read `docs/plans/inline-editor.md` before reopening it — it records
the rejected options and the settled shape should it ever be built, plus one
separable read-only idea (keyboard selection and yank, closing the gap where only
mouse users can copy part of a fragment).

### Search index follow-up

- [ ] Decide whether `MemoryIndex` needs an inverted index over titles and tags.
  Otherwise, rename it to reflect its linear scan. The answer depends on the
  supported library size and is not urgent at today's scale.

## Library format

### `doctor` diagnostics follow-up

The format refresh tightened new writes and left the reporting side deliberately
narrow, so old libraries stayed readable. Two gaps were named in that work and
never recorded here:

- [ ] Broader name-collision diagnostics: Unicode normalization, case-only
  differences, and Windows reserved names. `snip doctor` warns about
  non-portable folder names today, but does not detect two entries that collide
  only after normalization or only on a case-insensitive filesystem.
- [ ] Have `snip doctor` scan trash entries. `src/service/doctor.rs` walks the
  live catalog only, so a malformed package survives in `trash/` unreported and
  surfaces at restore time instead.

## Documentation

### Reconcile and archive the man page plans

PR #35 and PR #36 shipped the manual overhaul and the prose drift checks, but
neither plan document was closed out:

- [ ] `docs/plans/man-drift-followup.md` still reads `状态: 待实施` against
  baseline `fdf655a`, while `derived_from`, `--accept-sources`, and the
  `eol=lf` pins are all on `main`. Walk its 17 acceptance checks (and the 14 in
  `docs/plans/man-page-overhaul.md`) against what actually shipped, tick or
  re-open each, then move both files into `docs/plans/completed/`.
- [ ] Several other finished plans are still loose in `docs/plans/`
  (`editor-cwd.md`, `help-panel-redesign.md`, `narrow-terminal-layout.md`,
  `scheme-transposition-and-preview.md`, `user-defined-keys.md`). Archive them
  the same way so `docs/plans/` means "not done yet".

## Sharing

### GitLab snippets

- [ ] Publish through `glab`, reusing the `[[remotes]]` record in `snippet.toml`
  with a second `kind`, alongside the shipped `snip gist` command set.

## Data import

### SnippetsLab importer audit

`snip import snippetslab` (`src/importer/snippetslab/`) landed in 0.1.0 and has
only been refactored since — never re-validated against a current SnippetsLab
export. The committed fixture mirrors the object graph of a real 2.6 library, so
it guards decoder regressions but not upstream format changes; the decoder reads
`version.plist` only for the report and does not adapt to the format that version
describes.

- [ ] Import a real, current SnippetsLab library (both 1.x and 2.x exports) and
  confirm fields, folders, tags, fragments, and notes still land correctly;
  update the field mappings for any format the current decoder misses.

## Project homepage

A lightweight showcase site for snip, kept distinct from the README so the repo
stays the single source of truth.

- [ ] Build a static landing page: tagline, feature highlights, screenshots of
  the TUI, a one-line install command, and links to the README, manual pages, and
  the latest release.
- [ ] Derive the page content from `README.md` (generated or synced), so the copy
  never drifts from the repo.
- [ ] Serve it at a stable, branded URL (planned: `snip.gitkeniwo.tech`) and point
  the README badge links at it.

## Packaging

Existing channels: Homebrew, Nix (flake + Cachix), Copr, OBS (openSUSE and the
apt family), AUR (`sniplab` and `sniplab-bin`), Scoop, Gentoo, standalone
`.deb` / `.rpm` / archives, and `install.sh`.

### Open Build Service

The project builds natively for openSUSE Tumbleweed and Leap 16.0, Ubuntu
22.04/24.04/26.04, and Debian 13/testing/unstable. What remains lives on the OBS
side rather than in this repository, which is exactly why it needs writing down:

- [ ] Ubuntu 22.04 and 24.04 need the repository paths
  `Ubuntu:22.04/universe-update` and `Ubuntu:24.04/universe-update` configured
  in the OBS project's meta. This is project configuration, not a change any
  file here can carry, so a rebuilt project would silently lose it.

### Gentoo overlay

Gentoo users have a self-hosted overlay rather than a GURU submission, at least
to start.

- [x] Add `packaging/gentoo/ebuild-bin.in`, rendered by the release workflow
  the same way the Copr spec and the Scoop manifest are. Install the prebuilt
  release archive instead of compiling: building from crates.io requires either
  every dependency enumerated in `CRATES` (generated with `pycargoebuild`) or a
  vendored tarball, and neither earns its keep in an overlay. Install the binary,
  the shell completions, and `man/*.1`, mirroring the Copr spec's `%install`.
- [x] Publish it from a separate overlay repository (`metadata/layout.conf` plus
  `profiles/repo_name`), so users add it with `eselect repository add`.
- [ ] Register the overlay in `gentoo/api-gentoo-org`'s `repositories.xml` so it
  appears in `eselect repository list` without a manual URL.
- [ ] GURU is the eventual destination, not the starting point: it wants
  OpenPGP-signed commits carrying `Signed-off-by`, plus push access requested on
  IRC or the mailing list. Revisit once the overlay has been stable across a few
  releases.

### winget

Windows already ships `snip-x86_64-pc-windows-msvc.zip` for Scoop, and
winget-pkgs accepts a portable zip directly — no separate installer needed, and
there is no popularity threshold for new packages.

- [ ] Settle the package identifier (`Publisher.Package`, e.g. `gitkeniwo.snip`)
  before the first submission; it is effectively permanent once accepted.
- [ ] Add `packaging/winget/` manifests (version, locale, and installer YAML)
  pointing at the existing zip with `InstallerType: zip`,
  `NestedInstallerType: portable`, and `PortableCommandAlias: snip`.
- [ ] Generate and submit the first version with `wingetcreate new`, which hashes
  the asset, validates against the schema, and opens the PR against
  `microsoft/winget-pkgs`. First submissions get a human review; later ones are
  largely automated.
- [ ] Automate subsequent releases with `wingetcreate update ... --submit` in the
  release workflow, using a fork PAT, mirroring the existing Scoop bucket step.

### nixpkgs submission

- [ ] `nix/package.nix` is written to nixpkgs conventions and is ready to move to
  `pkgs/by-name/sn/sniplab/package.nix`. Before opening the PR:
  - [ ] run `nixfmt-rfc-style` on it, and fix everything `nixpkgs-hammering`
    reports
  - [ ] fill in `meta.maintainers`; a first-time submitter also has to add
    themselves to `maintainers/maintainer-list.nix`, either in the same PR or in
    one before it
- [ ] Refresh the `version` and both hashes in `nix/package.nix` once, right
  before opening the PR (the flake never reads them). They currently say
  `0.3.0`, three minor releases behind, which is harmless while nothing reads
  the file but is the first thing a reviewer would catch.
- [ ] Once it is in nixpkgs, users can install `pkgs.sniplab` without adding a
  flake input at all, and the flake in this repository stays for tracking `main`.

## Shipped

### TUI

- **Vendor-neutral GUI editor naming** — `gui_editor` and `snippet.open-gui`
  replace VS Code-specific vocabulary while retaining both old names as
  deprecated 0.x aliases. Palette search, help, and launch status resolve the
  configured executable name at runtime. (`docs/plans/completed/gui-editor-naming.md`)

- **Configurable editor working directory** — `editor_cwd` launches external
  edits from the inherited directory, the library, the containing folder, the
  snippet package, or the fragment's own directory, through one cwd-resolution
  path shared by the CLI and the TUI. Default stays `inherit`. Package-level
  directories can move under a concurrent save, so `folder` or `library` is the
  recommendation for concurrent writers. (`docs/plans/editor-cwd.md`)
- **Help panel redesign** — contextual and all-mode browsing with search over
  the effective keymap, readable selected rows, and keyboard hints that stay in
  sync with the generated key documentation.
  (`docs/plans/help-panel-redesign.md`)
- **Simplified UI mode** — a square, font-independent bar mode for terminals
  without powerline glyphs, reachable from `--simplified-ui`, the
  `tui.simplified_ui` config key, and an in-app toggle.
- **Search result legibility** — an active search is visible while it is
  running, input mode stays stable across refreshes, and matches stay
  highlighted inside truncated excerpts.
- **Narrow-terminal layout** — the session-only `S` sidebar toggle reclaims its
  width for reading, and snippet titles retain at least ten display cells before
  date, gist, and language decoration give way. (`docs/plans/narrow-terminal-layout.md`)
- **Session appearance override** — `A` toggles light/dark for the current TUI
  session by occupying the existing environment-precedence slot, so an explicit
  keypress beats `SNIP_TUI_THEME` without adding another resolver layer;
  **Clear Appearance Override** in the command palette restores normal theme
  resolution.
  (`docs/plans/completed/session-appearance-override.md`)

- **User-defined key bindings** — every TUI action has a stable name and can be
  rebound per mode in `keys.toml`; all in-app hints read the effective map, and
  `snip keys` lists, looks up, exports, and validates bindings with human or JSON
  output. (`docs/plans/user-defined-keys.md`)

- **Color themes** — seventeen built-in light and dark themes, editable TOML
  themes under the user config directory, `snip theme` inspection and switching,
  `tui-light-theme` / `tui-dark-theme` config keys, and a command-palette picker
  with live full-UI and syntax-highlighting preview. (0.3.2,
  `docs/plans/completed/color-schemes.md`)
- **Theme readability** — runtime `legible_on` fallback so every pill and
  retained-selection foreground picks a readable color at render time; the
  generator's contrast floors raised to WCAG 4.5 (with graphic floors for `rule`
  and `border`) and the built-ins regenerated; `theme check` rewritten to the
  real render pairs, with regression tests that no built-in fails.
  (`docs/plans/completed/theme-contrast.md`)
- **Theme palette refresh** — `light-default` is now the blue daylight sibling
  of `dark-default`, the previous teal palette remains as `light-teal`, and
  generated Base16 themes use accent-derived selection, retained-selection,
  and primary-pill surfaces. Bars now maintain a subtle 1.35:1 step from the
  canvas, secondary pills continue another 1.5:1 neutral step, and the three
  terminal-surface defaults share that generator path. The four screen-corner
  caps are flush while interior pills stay rounded.
  (`docs/plans/completed/theme-palette-refresh.md`,
  `docs/plans/completed/bar-pill-refinement.md`)
- **Theme import** — `snip theme import <scheme.yaml>` brings local base16/base24
  schemes in as editable user themes; the base16 → UI-role mapping, contrast
  clamping, and selection-foreground choice moved into a shared library, adding
  no runtime dependency. 29 curated built-ins, thirteen of them light, with
  hand-verifiable provenance. (`docs/plans/completed/theme-import.md`)
- **README as a preview item** — `PreviewTarget { Fragment(usize), Readme }`
  replaces `App.fragment_index` so the compiler forces an answer for every
  fragment-scoped action; `PreviewDocument` became an enum, making the appended
  README tail unrepresentable rather than merely deleted; the README is the last
  row of the fragment tree, drawn only when it exists and excluded from the
  fragment count; `[` / `]` wrap around the ends.
  (`docs/plans/completed/readme-preview-item.md`)
- **Grapheme-cluster-aware width** — `char_width` / `truncate_end` walk grapheme
  clusters instead of `chars()`, so ZWJ emoji measure and truncate as one unit;
  the prose/code wrapping in `src/tui/preview/layout.rs` stays correct, covered by
  a ZWJ regression test alongside the existing CJK ones.
  (`docs/plans/completed/grapheme-width.md`)
- **Fragment editing** — add, rename, reorder, and delete-with-confirmation from
  the preview's fragment tree, via `n.add` / `n.rename` / `n.reorder` /
  `n.remove` in the command registry.
  (`docs/plans/completed/tui-fragment-editing.md`)
- **Rendered-preview cache** — keyed on `(fingerprint, fragment_index, content
  width, show_line_numbers)`, removing the per-frame document clone, recompose,
  and rewrap in one change, with counted-allocation regression guards for both
  cache hits and the full draw path. Measurement found no leak (TUI RSS flat at
  11.5–11.8 MB; ~4.5 MB over the CLI, of which library content is ~0.8 MB), so
  total footprint was not worth optimizing. A lazy syntax set and
  visible-window-only highlighting were both considered and rejected.
  (`docs/plans/completed/preview-render-cost.md`)
- **Shared preview ownership** — `App` and `MemoryIndex` share one
  `Arc<CatalogSnapshot>`. Live and trash previews borrow snippets or clone
  `Arc`s, so frames do not copy snippet content.
  (`docs/plans/completed/preview-ownership-and-tabs.md`, Track 1)
- **Tab expansion in previews** — Tabs in fragment, README, and note previews
  expand to four-cell stops after syntax highlighting. Display-cell widths keep
  CJK wrapping and mouse selection aligned. Full-content copy preserves source
  tabs. (`docs/plans/completed/preview-ownership-and-tabs.md`, Track 2)

### Library portability

- **Runtime directories survive a clone** — `.snip/` subdirectories are all
  gitignored and Git records no empty directories, so a clone arrived without
  them and `Library::open` rejected the library outright; `open` now self-heals
  `snippets/`, `trash/`, `.snip/cache/`, `.snip/locks/`, and
  `.snip/transactions/`, best-effort so read-only mounts still read. No tracked
  `.gitkeep`, per FORMAT.md. (`docs/plans/completed/library-restore.md`)
- **Restoring on another machine** — `NO_LIBRARY_HINT` names the path for an
  existing or freshly cloned library, the onboarding wizard's connect branch
  accepts incomplete clones, and the flow is documented in the `snip-git` manual
  and the README. `default_library` deliberately stays in the user config rather
  than the library.
- **`snip git clone <remote> [path] [--gh] [--set-default]`** — the restore entry
  point beside `init` / `commit` / `backup` / `push` / `fetch`. Dispatched before
  library resolution, defaults to `~/<repo-name>.sniplib`, refuses a non-empty
  target, validates that the clone really is a library (deleting it if not), and
  delegates credentials to Git or `gh` without ever prompting.
- **Continuous sync** — safe `fetch + merge` in the Git core (repository-level
  dirty check, lock held for the workspace stage, immediate `merge --abort` on
  conflict); `snip git pull [--ff-only]` with stable human and JSON output and
  integration tests for the fast-forward, divergence, conflict-rollback, and
  refusal paths; `Ctrl-g l` to pull and `Ctrl-g U` to start auto-pull in the TUI,
  with an explicit catalog rescan on success. (`docs/plans/completed/library-pull.md`)

### Library format

- **The v1 specification matches the implementation again** — `FORMAT.md` was
  rewritten to document metadata fields, validation severity, atomic writes,
  fingerprints, gist payload digests, portable paths, trash, and transaction
  recovery, with a conformance fixture guarding it.
- **Strict on write, forgiving on read** — `open` now rejects only broken
  library identity and schema, while `doctor` reports malformed timestamps and
  remote metadata, so an old library stays readable. New writes refuse symlinks
  anywhere inside a snippet package and paths escaping trash or transactions,
  the UTF-8 filename limit is a correct 80 bytes, existing non-portable folders
  can still be read, edited, and renamed to a portable name, and a failed
  recovery names the backup rescue path.

### Documentation

- **Task-oriented manual** — 80 clap-generated pages became 15 substantive
  section-1 pages plus 16 compatibility stubs, four section-5 file-format
  pages, and `snip-agents(7)`, driven by a declarative page manifest. A new
  visible clap command that no page covers fails the generator's `--check`.
  `snip man path` prints the hierarchy root, `snip man generate DIR` writes
  `man1` / `man5` / `man7`, `snip man show` accepts `config.5` as well as a
  bare stem, and an upgrade prunes pages the previous version installed.
  (`docs/plans/man-page-overhaul.md`)
- **Prose drift checks** — key binding and config schema man content is
  generated from its source registries, and pages that transcribe prose from
  elsewhere record a `derived_from` digest, so editing `FORMAT.md` or
  `docs/keys.md` without revisiting the page fails CI with the exact
  `--accept-sources` command to run. Digests normalize CRLF and the sources are
  pinned `eol=lf`, so the check cannot fail spuriously on Windows.
  (`docs/plans/man-drift-followup.md`)

### Sharing

- **`snip gist`** — publishes a snippet to GitHub Gists through `gh`, with no
  embedded API client and no token handling. Fragments become gist files, the
  README becomes `README.md`, and the gist is recorded in `snippet.toml` so
  `push` keeps the same URL. Commands: `push`, `url`, `status`, `attach`,
  `detach`, `delete`, `open`. (`docs/plans/completed/gist-publish.md`)
- **Gist in the TUI** — a `Ctrl-s` panel for the selected snippet, palette
  entries for every gist command, a `Published` sidebar toggle that composes with
  the folder and tag filters, and list/preview markers that appear only for
  published snippets. (`docs/plans/completed/gist-tui.md`)

### Data import

- **SnippetsLab regression fixture** — a small, real-world fixture mirroring the
  object graph of a real 2.6 library (verified against one), rebuilt by the test
  builder rather than captured verbatim, plus a test that imports it.

### Packaging

- **Open Build Service packages** — tagged releases submit vendored,
  offline-buildable sources to `home:gitkeniwo/sniplab`, which compiles
  natively for openSUSE Tumbleweed and Leap 16.0, Ubuntu 22.04/24.04/26.04, and
  Debian 13/testing/unstable, covering Mint, Pop!_OS, Zorin, elementary, LMDE,
  and Kali through their bases, on x86_64 and arm64. `zypper` and `apt` install
  a prebuilt `snip` with manual pages and completions and upgrade normally. An
  `OBS packaging probe` workflow rebuilds the openSUSE rpm and the Debian source
  transform the way OBS does — offline, with an empty Cargo cache, so a broken
  vendor config cannot hide behind a warm registry — and `Release build` takes
  an `obs_only` dispatch that resubmits the current version from `main` without
  cutting a release. Verified against the real project by two such dispatches.
- **Gentoo overlay** — `app-misc/sniplab-bin` installs the static musl release
  binary, `man/*`, and shell completions from a standalone Portage overlay;
  tagged releases render its ebuild and thin Manifest the same way the Copr spec
  and Scoop manifest are rendered.
- **Windows without a redistributable** — MSVC builds link the static CRT, and
  a shared `dumpbin /dependents` gate fails CI on any dynamic VC/MSVC/UCRT
  import, checked both in normal CI and before the release ZIP is assembled, so
  the portable Windows asset runs without a separately installed Visual C++
  runtime.
- **Binary cache** — a Cachix cache named `snip` with `CACHIX_AUTH_TOKEN` in the
  repo secrets, so the `Nix` and `Release build` workflows push prebuilt binaries
  and `nix run` / `nix profile install` download instead of building. Verified on
  x86_64-linux, aarch64-linux, and aarch64-darwin. (`docs/plans/completed/nix-flake.md`)
- **Arch binary package** — `sniplab-bin` on the AUR, so Arch users can
  `yay -S sniplab-bin` without compiling. The release `aur` job renders both the
  source and prebuilt PKGBUILDs, and the portable Linux archives carry `LICENSE`
  and `README.md` for the package to install.
- **Standalone install script** — `install.sh` fetches the release asset for the
  host platform into `~/.local/bin`, needing no `sudo`; resolves the target
  triple through a small `uname` table so new targets slot in by adding a row,
  falling back to `cargo install sniplab` when there is no prebuilt asset;
  idempotent and upgrade-safe (prints old and new versions, refuses to downgrade,
  leaves package-manager-owned installs untouched); ships an `uninstall` mode and
  is published at the stable `releases/latest/download/install.sh` URL.
  (`docs/plans/completed/install-script.md`)

### Skill

- **Note vs. README guidance** — the choosing rule ("prose about *this one file*
  is a note, prose about *the set* is the README") is stated in `skills/snip`
  and the `snip create` help, so agents no longer file whole-snippet prose in
  the single fragment's note.
