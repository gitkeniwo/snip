%%DESCRIPTION
Create a new .sniplib library or import an existing SnippetsLab collection. init can create a dedicated Git repository and record a human-readable library name. In noninteractive use, pass an explicit path or --yes.

SnippetsLab imports require a destination library. Run with --dry-run first to inspect counts, normalized tags, attachments, and validation warnings without modifying the destination.

%%EXAMPLES
Create a Git-backed library without prompting:

    snip init ~/Notes.sniplib --name Notes --git --yes

Preview and then perform a SnippetsLab import:

    snip import snippetslab SnippetsLab.json --into ~/Imported.sniplib --dry-run
    snip import snippetslab SnippetsLab.json --into ~/Imported.sniplib

%%SEE ALSO
snip(1), snip-doctor(1), snip-git(1), sniplib(5)
