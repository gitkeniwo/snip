%%DESCRIPTION
Open the interactive terminal browser for the resolved library. The TUI provides browsing, preview, editing, trash, Git, Gist, theme, and key-help workflows without changing the underlying library format.

Bindings are loaded from keys.toml beside config.toml. Use snip keys list to inspect effective bindings and snip keys export to create an editable starting point. The TUI requires a terminal and fails fast when standard input or output is not interactive.

%%EXAMPLES
Open the resolved library:

    snip tui

Open a named library with square bar caps for terminals without Powerline glyphs:

    snip --library ~/Work.sniplib --simplified-ui tui

%%SEE ALSO
snip(1), snip-keys(1), snip-theme(1), snip-keys(5), snip-theme(5)
