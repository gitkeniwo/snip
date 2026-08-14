%%DESCRIPTION
Create a snippet with one initial fragment. The title is required; language, folder, and tags fall back to configured defaults when omitted. --tag is repeatable, and a missing folder is created automatically.

Inline text and file input are mutually exclusive. A path of - for --content-file, --note-file, or --readme-file reads UTF-8 text from standard input. A note describes the initial fragment; a README describes the snippet as a whole.

%%EXAMPLES
Create a shell snippet from inline content:

    snip create --title "List ports" --language bash --tag network --content "lsof -i"

Create a snippet from standard input:

    pbpaste | snip create --title "tar over ssh" --content-file -

%%SEE ALSO
snip(1), snip-edit(1), snip-query(1), sniplib(5)
