%%SYNOPSIS
    snip [OPTIONS] <SELECTOR>

%%DESCRIPTION
snip stores code snippets as ordinary files in a .sniplib directory. It provides a scriptable command-line interface and an optional interactive terminal browser while keeping the filesystem as the source of truth.

Library resolution follows this order: --library, SNIP_LIBRARY, the nearest ancestor containing snip.toml, then default_library from the user configuration. Commands that need a library fail instead of silently creating one.

For quick human lookup, snip <selector> is equivalent to snip preview <selector>, including headers. Options belong on the explicit preview command, and a title that matches a subcommand name must be opened with snip preview because command names take precedence.

%%EXIT STATUS
0 indicates success.

1 is io_error: a filesystem, process, or operating-system operation failed.

2 is usage_error: command-line input is invalid or a required confirmation is missing.

3 is no_library or not_found: no library could be resolved, the requested item does not exist, or the selector matches more than one snippet.

4 is conflict: the target is locked, changed since it was read, or would overwrite unmanaged data.

5 is validation_error: input or managed data violates snip's format rules.

%%ENVIRONMENT
SNIP_LIBRARY selects a library when --library is absent.

SNIP_TUI_THEME temporarily selects the TUI theme.

SNIP_GH_BIN overrides the gh executable used by gist commands.

EDITOR and VISUAL select the external text editor. PAGER selects the preview pager. NO_COLOR disables color when color policy is auto.

XDG_CONFIG_HOME controls the user configuration directory. XDG_DATA_HOME controls the default user man-page installation root.

%%FILES
~/.config/snip/config.toml is the default user configuration file. keys.toml beside it contains TUI key bindings.

~/.local/share/snip/man-install.json records user-installed manual pages when XDG_DATA_HOME is unset.

*.sniplib is a snippet library. Its format is documented in sniplib(5).

%%EXAMPLES
List snippets from the resolved library as JSON:

    snip --output json list

Open a specific library in the terminal browser:

    snip --library ~/Notes.sniplib tui

%%SEE ALSO
snip-query(1), snip-create(1), snip-edit(1), snip-trash(1), sniplib(5), snip-config(5), snip-agents(7)
