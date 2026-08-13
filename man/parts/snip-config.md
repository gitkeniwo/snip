%%DESCRIPTION
Inspect and modify the user configuration. path prints the resolved config file, show prints effective stored values, init creates a file, set updates one supported key, and unset removes a key so the built-in default applies again.

Unknown TOML fields are preserved when snip writes the file. Configuration key meanings and defaults are documented in snip-config(5).

%%EXAMPLES
Print the configuration path and current contents:

    snip config path
    snip --output json config show

Set a default library and remove the saved color policy:

    snip config set default-library ~/Notes.sniplib
    snip config unset color

%%SEE ALSO
snip(1), snip-config(5), snip-keys(1), snip-theme(1)
