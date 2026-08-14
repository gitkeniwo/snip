%%DESCRIPTION
Inspect, export, and validate the effective TUI key map. list groups bindings by mode, show finds every binding for an action slug, path prints the keys.toml location, export writes an authoritative default file, and check reports conflicts and lockouts.

Key bindings are separate from config.toml and are never rewritten by snip config set. The file format, modes, chords, and required escape bindings are documented in snip-keys(5).

%%EXAMPLES
List effective preview-mode bindings and inspect one action:

    snip keys list --mode preview
    snip keys show copy-content

Create an editable key map and validate it:

    snip keys export
    snip keys check

%%SEE ALSO
snip(1), snip-tui(1), snip-config(1), snip-keys(5)
