%%DESCRIPTION
keys.toml customizes rebindable TUI actions by mode. Run snip keys path to locate it, snip keys export to create a complete authoritative starting point, snip keys list to inspect the effective map, and snip keys check after editing.

%%FILE FORMAT
The top-level inherit-defaults boolean defaults to true. Each configurable mode is a TOML table whose keys are action slugs and whose values are one chord string or a list of chord strings. An explicit empty list unbinds an action. A complete export sets inherit-defaults = false.

    inherit-defaults = true

    [list]
    "snippet.edit-content" = "e"
    "snippet.rename" = ["r", "f2"]
    "snippet.move" = []

Mentioned actions lose their previous bindings before replacements are inserted, allowing two keys to be swapped. Duplicate claims in one mode are rejected together. ctrl-c is reserved. Unknown modes, action slugs, chord names, and configurable search mode entries are errors.

%%MODES
Normal pane stacks include global and the focused pane mode. Exclusive fragment-grab, trash, help, git, gist, and search modes take over input but inherit a small allowlist from global. When several modes are active, the narrower mode wins.

Configurable modes are global, sidebar, list, preview, fragment, fragment-grab, trash, help, git, and gist. Search text editing, digits, ctrl-c, and mouse gestures are fixed rather than rebindable.


%%SEE ALSO
snip(1), snip-tui(1), snip-keys(1), snip-config(5)
