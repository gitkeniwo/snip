# Roadmap

Planned work, roughly in the order it is likely to land. Nothing here is a
commitment to a date.

Open work comes first, grouped by area. [Shipped](#shipped) at the bottom is a
one-line-per-item record — the design rationale behind those items lives in
`docs/plans/` and in the commit history, not here.

## TUI

### User-defined key bindings

- [ ] Make every TUI action rebindable through the config file, with the current
  bindings as the default set. Needs a stable action name for each command and a
  way to show the effective bindings (both in the help pane and as JSON).

### Configurable snippet editor

- [ ] Let the user pick per preference: an in-TUI editor (`tui-textarea`) for
  quick edits without leaving the browser, `$EDITOR` for terminal editors, or an
  external GUI editor launched detached.
- [ ] The choice belongs in the config file, with a sensible fallback chain when
  the configured editor is missing.

### Tab expansion in the preview

- [ ] Expand tabs before syntax highlighting using an explicit tab-stop policy;
  ratatui filters the control character itself, so tabs currently collapse to one
  visual column. Independent of the grapheme-cluster measurement that shipped
  alongside it.

### Preview allocation follow-up

The rendered-preview cache removed the per-frame recompose and rewrap. One clone
per frame remains:

- [ ] Borrow the selected snippet in `draw_preview` rather than cloning it
  (`src/tui/preview/render.rs:21`). The clone exists to end the `&App` borrow
  before `&mut App` is needed, so this needs the borrow split untangled first.
  Deliberately deferred after the cache work: the normal and trash preview
  ownership paths need a wider borrow refactor, for a much smaller payoff than
  the wrapping allocations already removed.
  Tracks `docs/plans/preview-render-cost.md`.

### Catalog held twice in memory

`App` holds a `CatalogSnapshot` and a `MemoryIndex` that is nothing but a second
full copy of it (`src/tui/app/core.rs:51` and `:496` call
`MemoryIndex::new(catalog.clone())`; `src/search.rs:106` shows the struct has one
field and builds no index — `search()` is a linear scan over `catalog.snippets`).
A rescan transiently holds a third copy. That is ~0.8 MB at today's scale, but it
grows linearly with library size.

- [ ] Share one `Arc<CatalogSnapshot>` between `App` and `MemoryIndex`, or have
  the search borrow the catalog directly, so the snapshot is stored once.
- [ ] Separately: `MemoryIndex` is misnamed for what it does. Decide whether it
  should acquire a real index (inverted index over titles/tags, or a prefilter)
  or be renamed to reflect that it is a linear scanner. This depends on the
  library size we intend to support and is not urgent at today's scale.

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

Existing channels: Homebrew, Nix (flake + Cachix), Copr, AUR (`sniplab` and
`sniplab-bin`), Scoop, standalone `.deb` / `.rpm` / archives, and `install.sh`.

### Gentoo overlay

Gentoo users currently have `install.sh` and nothing native. The plan is a
self-hosted overlay rather than a GURU submission, at least to start.

- [ ] Add `packaging/gentoo/sniplab.ebuild.in`, rendered by the release workflow
  the same way the Copr spec and the Scoop manifest are. Install the prebuilt
  release archive instead of compiling: building from crates.io requires either
  every dependency enumerated in `CRATES` (generated with `pycargoebuild`) or a
  vendored tarball, and neither earns its keep in an overlay. Install the binary,
  the shell completions, and `man/*.1`, mirroring the Copr spec's `%install`.
- [ ] Publish it from a separate overlay repository (`metadata/layout.conf` plus
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
  before opening the PR (the flake never reads them).
- [ ] Once it is in nixpkgs, users can install `pkgs.sniplab` without adding a
  flake input at all, and the flake in this repository stays for tracking `main`.

## Shipped

### TUI

- **Color themes** — seventeen built-in light and dark themes, editable TOML
  themes under the user config directory, `snip theme` inspection and switching,
  `tui-light-theme` / `tui-dark-theme` config keys, and a command-palette picker
  with live full-UI and syntax-highlighting preview. (0.3.2,
  `docs/plans/color-schemes.md`)
- **Theme readability** — runtime `legible_on` fallback so every pill and
  retained-selection foreground picks a readable color at render time; the
  generator's contrast floors raised to WCAG 4.5 (with graphic floors for `rule`
  and `border`) and the built-ins regenerated; `theme check` rewritten to the
  real render pairs, with regression tests that no built-in fails.
  (`docs/plans/theme-contrast.md`)
- **Theme import** — `snip theme import <scheme.yaml>` brings local base16/base24
  schemes in as editable user themes; the base16 → UI-role mapping, contrast
  clamping, and selection-foreground choice moved into a shared library, adding
  no runtime dependency. 28 curated built-ins, twelve of them light, with
  hand-verifiable provenance. (`docs/plans/theme-import.md`)
- **README as a preview item** — `PreviewTarget { Fragment(usize), Readme }`
  replaces `App.fragment_index` so the compiler forces an answer for every
  fragment-scoped action; `PreviewDocument` became an enum, making the appended
  README tail unrepresentable rather than merely deleted; the README is the last
  row of the fragment tree, drawn only when it exists and excluded from the
  fragment count; `[` / `]` wrap around the ends.
  (`docs/plans/readme-preview-item.md`)
- **Grapheme-cluster-aware width** — `char_width` / `truncate_end` walk grapheme
  clusters instead of `chars()`, so ZWJ emoji measure and truncate as one unit;
  the prose/code wrapping in `src/tui/preview/layout.rs` stays correct, covered by
  a ZWJ regression test alongside the existing CJK ones.
  (`docs/plans/grapheme-width.md`)
- **Fragment editing** — add, rename, reorder, and delete-with-confirmation from
  the preview's fragment tree, via `n.add` / `n.rename` / `n.reorder` /
  `n.remove` in the command registry.
  (`docs/plans/tui-fragment-editing.md`)
- **Rendered-preview cache** — keyed on `(fingerprint, fragment_index, content
  width, show_line_numbers)`, removing the per-frame document clone, recompose,
  and rewrap in one change, with counted-allocation regression guards for both
  cache hits and the full draw path. Measurement found no leak (TUI RSS flat at
  11.5–11.8 MB; ~4.5 MB over the CLI, of which library content is ~0.8 MB), so
  total footprint was not worth optimizing. A lazy syntax set and
  visible-window-only highlighting were both considered and rejected.
  (`docs/plans/preview-render-cost.md`)

### Library portability

- **Runtime directories survive a clone** — `.snip/` subdirectories are all
  gitignored and Git records no empty directories, so a clone arrived without
  them and `Library::open` rejected the library outright; `open` now self-heals
  `snippets/`, `trash/`, `.snip/cache/`, `.snip/locks/`, and
  `.snip/transactions/`, best-effort so read-only mounts still read. No tracked
  `.gitkeep`, per FORMAT.md. (`docs/plans/library-restore.md`)
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
  with an explicit catalog rescan on success. (`docs/plans/library-pull.md`)

### Sharing

- **`snip gist`** — publishes a snippet to GitHub Gists through `gh`, with no
  embedded API client and no token handling. Fragments become gist files, the
  README becomes `README.md`, and the gist is recorded in `snippet.toml` so
  `push` keeps the same URL. Commands: `push`, `url`, `status`, `attach`,
  `detach`, `delete`, `open`. (`docs/plans/gist-publish.md`)
- **Gist in the TUI** — a `Ctrl-s` panel for the selected snippet, palette
  entries for every gist command, a `Published` sidebar toggle that composes with
  the folder and tag filters, and list/preview markers that appear only for
  published snippets. (`docs/plans/gist-tui.md`)

### Data import

- **SnippetsLab regression fixture** — a small, real-world fixture mirroring the
  object graph of a real 2.6 library (verified against one), rebuilt by the test
  builder rather than captured verbatim, plus a test that imports it.

### Packaging

- **Binary cache** — a Cachix cache named `snip` with `CACHIX_AUTH_TOKEN` in the
  repo secrets, so the `Nix` and `Release build` workflows push prebuilt binaries
  and `nix run` / `nix profile install` download instead of building. Verified on
  x86_64-linux, aarch64-linux, and aarch64-darwin. (`docs/plans/nix-flake.md`)
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
  (`docs/plans/install-script.md`)

### Skill

- **Note vs. README guidance** — the choosing rule ("prose about *this one file*
  is a note, prose about *the set* is the README") is stated in `skills/snip`
  and the `snip create` help, so agents no longer file whole-snippet prose in
  the single fragment's note.
