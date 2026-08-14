%%DESCRIPTION
snip user configuration is TOML schema version 1. It supplies machine-local defaults across libraries; it never travels as part of a .sniplib directory. Unknown fields are preserved when snip config set or unset rewrites the file.

%%LOCATION
The file is $XDG_CONFIG_HOME/snip/config.toml, or ~/.config/snip/config.toml when XDG_CONFIG_HOME is unset. Explicit command-line options override configuration. SNIP_TUI_THEME and an in-session appearance override may override theme selection.


%%EDITOR WORKING DIRECTORY
editor_cwd is inherit, library, folder, snippet, or fragment. inherit keeps the caller directory. library uses the .sniplib root. folder uses the package's containing folder. snippet uses the package. fragment uses the fragment or note directory; README and metadata stay at package level.

Snippet packages are transactionally replaced as whole directories. Prefer folder or library when another process may write while an editor is open. On Windows, configure an absolute editor executable for untrusted libraries; relative commands are resolved before changing directory.


%%SEE ALSO
snip(1), snip-config(1), snip-tui(1), snip-theme(5), snip-keys(5)
