%%DESCRIPTION
List, inspect, validate, export, import, and select TUI color themes. Built-in and user themes share the same validation rules. Appearance slots allow separate light and dark choices.

export copies a theme into the user theme directory for editing. import converts a base16 or base24 scheme. check reports contrast and role-distinctness problems before a theme is selected.

%%EXAMPLES
List dark themes and inspect one as JSON:

    snip theme list --appearance dark
    snip --output json theme show dracula

Export a theme for editing and validate the result:

    snip theme export dracula --as dracula-custom
    snip theme check dracula-custom

%%SEE ALSO
snip(1), snip-tui(1), snip-config(1), snip-theme(5)
