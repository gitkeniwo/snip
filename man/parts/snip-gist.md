%%DESCRIPTION
Publish snippets through the GitHub gh CLI. push creates or updates a gist, url prints its link, status compares local content with the last push, attach adopts an existing gist, detach forgets the local record, delete removes the remote gist, and open launches a browser.

Gists are secret unless --public is used at creation. GitHub cannot change an existing gist's visibility. --new creates a different gist and leaves the old one online; gist delete is irreversible. These operations require gh to be installed and authenticated.

%%EXAMPLES
Create or update a secret gist and print its URL:

    snip gist push Brewfile
    snip gist url Brewfile

Check every linked snippet without network access, then check one remotely:

    snip --output json gist status --all
    snip gist status Brewfile --remote

%%SEE ALSO
snip(1), snip-query(1), snip-edit(1), snip-agents(7), sniplib(5)
