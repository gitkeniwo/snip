# Roadmap

Planned work, roughly in the order it is likely to land. Nothing here is a
commitment to a date.

## TUI

### Color schemes

Let the theme be chosen and defined by the user rather than compiled in.
Ship a few built-in schemes, read a user scheme from the config file, and
respect terminal light/dark where possible.

### User-defined key bindings

Make every TUI action rebindable through the config file, with the current
bindings as the default set. Needs a stable action name for each command and
a way to show the effective bindings (both in the help pane and as JSON).

### Configurable snippet editor

Today editing goes through one path. Let the user pick per preference:

- an in-TUI editor (`tui-textarea`), for quick edits without leaving the browser
- `$EDITOR`, for terminal editors
- an external GUI editor, launched detached

The choice belongs in the config file, with a sensible fallback chain when the
configured editor is missing.

## Sharing

### Publish a snippet as a Gist

`snip` gains a command to publish a single snippet to GitHub or GitLab as a
gist, shelling out to `gh` (and `glab`) rather than embedding an API client or
handling tokens. Decisions still open: whether fragments become multiple gist
files, whether the gist URL is recorded back into the snippet, and what update
and delete look like.

## Packaging

### nixpkgs submission

`nix/package.nix` is written to nixpkgs conventions and is ready to move to
`pkgs/by-name/sn/sniplab/package.nix`. Before opening the PR:

- run `nixfmt-rfc-style` on it, and fix everything `nixpkgs-hammering` reports
- fill in `meta.maintainers`; a first-time submitter also has to add themselves
  to `maintainers/maintainer-list.nix`, either in the same PR or in one before it

Once it is in nixpkgs, users can install `pkgs.sniplab` without adding a flake
input at all, and the flake in this repository stays for tracking `main`.

### Binary cache

The `Nix` workflow already has the Cachix steps, guarded so they no-op without
`CACHIX_AUTH_TOKEN`. Creating a cache named `snip` and setting that secret turns
`nix run` / `nix profile install` from a source build into a download.
