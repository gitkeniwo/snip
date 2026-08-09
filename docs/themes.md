# TUI themes

`snip` ships light and dark themes and reads editable TOML themes from the
directory printed by `snip theme path`. A user file shadows a built-in theme
with the same name. Use `snip theme list`, `show`, `check`, `export`, and `use`
to inspect and manage them.

## File format

Every standalone theme declares schema version 1, a kebab-case name matching
its filename, an appearance, all UI roles, and a syntax strategy:

```toml
schema_version = 1
name = "my-dark"
display_name = "My Dark Theme"
appearance = "dark"
source = "personal"

[ui]
background = "#282828"
foreground = "#d5c4a1"
accent = "#83a598"
accent_alt = "#d3869b"
border = "#665c54"
muted = "#bdae93"
selection_bg = "#504945"
selection_fg = "#fbf1c7"
retained_bg = "#3c3836"
pill_primary = "#8ec07c"
pill_secondary = "#665c54"
bar_bg = "#3c3836"
bar_fg = "#d5c4a1"
tag = "#fe8019"
rule = "#504945"
success = "#b8bb26"
warning = "#fabd2f"
error = "#fb4934"

[syntax]
theme = "GruvboxDark"
```

`display_name` defaults to `name`; `source` is optional provenance. Names use
lowercase ASCII letters and digits separated by single hyphens. An appearance
is always `light` or `dark` and determines which config slot the picker saves.

Colors accept `#rgb`, `#rrggbb`, `ansi:0` through `ansi:255`, or one of the 16
ANSI names (`black`, `red`, …, `white`, and their `bright-` forms). Names and
hex digits are case-insensitive. `terminal` is legal for `background`,
`foreground`, and `pill_secondary`; the first two roles must either both be
`terminal` or both be set. A terminal secondary pill inherits the terminal's
canvas colour.

## UI roles

| Role | Used for |
| --- | --- |
| `background` | Full-screen and popup surface |
| `foreground` | Default text on that surface |
| `accent` | Focus, primary actions, and active elements |
| `accent_alt` | Secondary highlighted metadata |
| `border` | Unfocused pane and popup borders |
| `muted` | De-emphasized labels, counts, and hints |
| `selection_bg` | Active selected-row background |
| `selection_fg` | Active selected-row text |
| `retained_bg` | Selection retained in an unfocused pane |
| `pill_primary` | Primary shortcut pills |
| `pill_secondary` | Secondary shortcut pills |
| `bar_bg` | Top and bottom bar background |
| `bar_fg` | Top and bottom bar text |
| `tag` | Tag markers and labels |
| `rule` | Dividers and rules |
| `success` | Successful Git and status state |
| `warning` | Warnings and inline code accents |
| `error` | Errors and destructive state |

Text on `pill_primary`, `pill_secondary`, `retained_bg`, and selected
fragment-tree rows is drawn with its foreground computed at render time: the
preferred role colour is kept when it clears 4.5:1 on that surface, otherwise
the most legible of the theme's own colours is used, falling back to black or
white. Secondary shortcut and sort labels prefer the neutral `bar_fg`; primary
pills and status labels retain their accent or semantic colours. `theme check`'s
`computed-foreground` finding flags themes whose surfaces would force a
fallback; it is reported as a `note` rather than a warning because the TUI
handles the fallback automatically.

## Syntax highlighting

`[syntax]` contains exactly one choice. `theme` names an embedded two-face
theme, matched case-insensitively. Alternatively, derive highlighting from a
complete base16 palette:

```toml
[syntax]
derive = "base16"

[palette]
base00 = "#181818"
# base01 through base0E
base0F = "#7b3b3b"
```

All 16 keys from `base00` through `base0F` are required and must be six-digit
RGB literals. Derived syntax uses `base00` as background, `base05` as foreground
and caret, `base02` as selection, `base03` for comments and gutter text,
`base0B` for strings, `base09` for numbers, `base08` for variables/tags/errors,
`base0E` for keywords, `base0D` for functions/headings, `base0A` for types, and
`base0C` for constants, escapes, and regular expressions.

The generated built-ins map base16 slots to UI roles as follows:

| Roles | Slot |
| --- | --- |
| `background` | `base00` |
| `foreground`, `bar_fg` | `base05` |
| `accent` | `base0D` |
| `accent_alt` | `base0E` |
| `border` | `base03` |
| `muted` | `base04` |
| `selection_bg` | contrast-adjusted `base0D` mixed another 5% toward black or white |
| `rule` | `base02` |
| `retained_bg` | `base0D` mixed 90% toward `base00` |
| `bar_bg` | `base01`, mixed away from `base00` until they contrast by at least 1.35:1 |
| `pill_primary` | `base0D` mixed 20% toward `base00`, then adjusted to contrast 4.5:1 with `base00` |
| `pill_secondary` | adjusted `bar_bg`, mixed another step away from `base00` until it contrasts with `bar_bg` by at least 1.5:1 |
| `tag` | `base09` |
| `success` | `base0B` |
| `warning` | `base0A` |
| `error` | `base08` |

`dark-default`, `light-default`, and `light-teal` use the same surface
derivation. Their curated source files keep the semantic Primer-style roles and
provide an unadjusted bar source; generation computes bar and pill colours
against an assumed terminal canvas (`#0d1117` for dark, `#ffffff` for light),
then preserves `background = "terminal"` and `foreground = "terminal"` in the
output.

`selection_fg` is whichever of `base00`, `base05`, `base06`, or `base07` has
the highest WCAG contrast against the adjusted `base0D` selection background.
The current construction guarantees that `base00` clears 4.5:1: `accent` is
first contrast-adjusted against `base00`, then `selection_bg` moves another 5%
away from the background. The converter retains a defensive hard failure in
case a future mapping change breaks that invariant. When a mapped semantic
foreground (`muted`,
`bar_fg`, `tag`, `accent`, `accent_alt`, `warning`, `error`, `success`) is
below 4.5:1 on its background, generation moves it the minimum RGB blend
toward black or white needed to clear that floor; `rule` clears 3.0:1 and
`border` 2.5:1. The original 16-color palette remains recorded unchanged.

## Importing base16 schemes

Convert a local base16 or base24 scheme directly into an editable user theme:

```bash
snip theme import scheme.yaml
snip theme import scheme.yaml --as my-theme --syntax GruvboxDark
snip theme import scheme.yaml --dry-run > my-theme.toml
```

`--as` overrides the lowercased, hyphenated file-stem name; it is required
when the input path is `-` (stdin). `--syntax` uses an embedded syntax theme
instead of palette-derived highlighting, `--force` replaces an existing file,
and `--dry-run` prints the converted TOML without writing it.

The importer accepts the flat subset used by Tinted Theming: top-level `name`,
optional `variant`, and a `palette` block containing `base00` through `base0F`.
Unknown metadata and extra base24 slots are ignored. Indentation must use
spaces. A leading YAML document separator (`---`) is accepted. Unlike YAML, an
unquoted `base00: #2e3440` is treated as a color rather than a comment. A
quoted value ends at its closing quote, so it may be followed by an inline
comment; inline comments are not stripped from unquoted values.

The converter maps palette slots to UI roles and raises only the roles with a
listed contrast floor, blending toward black or white when needed:

| Role | Slot | Clamp |
| --- | --- | --- |
| `background` | `base00` | — |
| `foreground` | `base05` | — |
| `accent` | `base0D` | 4.5 vs `base00` |
| `accent_alt` | `base0E` | 4.5 vs `base00` |
| `border` | `base03` | 2.5 vs `base00` |
| `muted` | `base04` | 4.5 vs `base00` |
| `selection_bg` | contrast-adjusted `base0D` mixed another 5% toward black or white | 4.5 vs `base00` |
| `selection_fg` | best of `base00`, `base05`, `base06`, `base07` | — |
| `retained_bg` | `base0D` mixed 90% toward `base00` | — |
| `pill_primary` | `base0D` mixed 20% toward `base00` | 4.5 vs `base00` |
| `pill_secondary` | adjusted `bar_bg`, mixed farther toward white (dark) or black (light) | 1.5 vs adjusted `bar_bg` |
| `bar_bg` | `base01`, mixed toward white (dark) or black (light) | 1.35 vs `base00` |
| `bar_fg` | `base05` | 4.5 vs adjusted `bar_bg` |
| `tag` | `base09` | 4.5 vs `base00` |
| `rule` | `base02` | 3.0 vs `base00` |
| `success` | `base0B` | 4.5 vs `base00` |
| `warning` | `base0A` | 4.5 vs `base00` |
| `error` | `base08` | 4.5 vs `base00` |

Validation failures block an import; warnings are printed but the valid theme
is still saved. Run `snip theme check NAME` to see every finding afterward.

## Extending a theme

An extending theme still declares its own name and appearance, but may override
only the fields it needs. Everything else, including syntax and palette,
inherits from the resolved parent. `[ui]` may be left out entirely — a theme
that only swaps `[syntax]` is valid:

```toml
schema_version = 1
name = "dark-gruvbox-blue"
display_name = "Gruvbox Dark, Blue Accent"
appearance = "dark"
extends = "dark-gruvbox"

[ui]
accent = "#7daea3"
pill_primary = "#7daea3"
```

User themes shadow built-ins during inheritance. Cycles are rejected, and an
inheritance chain may contain at most eight parent links. Run
`snip theme check NAME` after editing: failed contrast checks prevent runtime
use, while warnings are shown without blocking the theme. The two failure
checks are `foreground-contrast` and `selection-contrast` (both 4.5:1); the
warning checks are `role-legibility` (4.5:1 on `background` and `bar_bg`) and
`graphic-legibility` (`rule` 3.0:1, `border` 2.5:1); the
`computed-foreground` check is a `note` because the TUI falls back to
black/white automatically rather than failing or warning the user.
