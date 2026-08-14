%%DESCRIPTION
Generate a completion script for a supported shell. The script is written to standard output; source it for the current session or save it in the shell's completion directory.

Completion generation does not modify shell startup files.

%%EXAMPLES
Load Bash completions for the current session:

    source <(snip completion bash)

Install a Zsh completion file in a user directory:

    mkdir -p ~/.zfunc
    snip completion zsh > ~/.zfunc/_snip

%%SEE ALSO
snip(1), snip-config(1), snip-man(1)
