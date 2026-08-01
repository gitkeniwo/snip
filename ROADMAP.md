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

### Configurable snippet editor

- [ ] Let the user pick per preference:
  - an in-TUI editor (`tui-textarea`), for quick edits without leaving the
    browser
  - `$EDITOR`, for terminal editors
  - an external GUI editor, launched detached
- [ ] The choice belongs in the config file, with a sensible fallback chain
  when the configured editor is missing.

## Sharing

### Publish a snippet as a Gist

- [x] `snip gist` publishes a snippet to GitHub Gists through `gh`, with no
  embedded API client and no token handling. Fragments become gist files, the
  README becomes `README.md`, and the gist is recorded in `snippet.toml` so
  `push` keeps the same URL. `push`, `url`, `status`, `attach`, `detach`,
  `delete`, and `open` are the command set.
- [ ] GitLab snippets through `glab`, reusing the `[[remotes]]` record with a
  second `kind`.
- [ ] TUI integration: publish and copy-link from the command palette, and a
  marker in the snippet list for published snippets.

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
