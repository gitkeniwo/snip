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
hex digits are case-insensitive. `terminal` is legal only for `background` and
`foreground`, and those two roles must either both be `terminal` or both be set.

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
| `border`, `pill_secondary` | `base03` |
| `muted` | `base04` |
| `selection_bg`, `rule` | `base02` |
| `retained_bg`, `bar_bg` | `base01` |
| `pill_primary` | `base0C` |
| `tag` | `base09` |
| `success` | `base0B` |
| `warning` | `base0A` |
| `error` | `base08` |

`selection_fg` is whichever of `base00`, `base05`, `base06`, or `base07` has
the highest WCAG contrast against `base02`; built-in generation stops if the
best option is below 4.5:1. When a mapped semantic foreground is below 3.0:1,
generation moves it the minimum RGB blend toward black or white needed to
clear that floor; the original 16-color palette remains recorded unchanged.

## Extending a theme

An extending theme still declares its own name and appearance, but may override
only the fields it needs. Everything else, including syntax and palette,
inherits from the resolved parent:

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
use, while warnings are shown without blocking the theme.
