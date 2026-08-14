%%DESCRIPTION
Publish snippets through the GitHub gh CLI. push creates or updates a gist, url prints its link, status compares local content with the last push, attach adopts an existing gist, detach forgets the local record, delete removes the remote gist, and open launches a browser.

Gists are secret unless --public is used at creation. GitHub cannot change an existing gist's visibility. --new creates a different gist and leaves the old one online; gist delete is irreversible. These operations require gh to be installed and authenticated.

After a successful push, snip records the filenames it owns, the note and README inclusion choices, pushed_at, and a pushed_digest of the canonical publication payload. Local status rebuilds that digest to report clean or modified without contacting GitHub. A matching push is skipped as unchanged unless --force is supplied; an attached gist has no pushed digest until it is pushed by snip.

The digest sorts published filenames in ascending UTF-8 byte order and covers their length-prefixed names and contents followed by the description. Its 32-byte BLAKE3 value is stored as exactly 64 lowercase hexadecimal characters. See sniplib(5) for the complete contract.

%%EXAMPLES
Create or update a secret gist and print its URL:

    snip gist push Brewfile
    snip gist url Brewfile

Check every linked snippet without network access, then check one remotely:

    snip --output json gist status --all
    snip gist status Brewfile --remote

%%SEE ALSO
snip(1), snip-query(1), snip-edit(1), snip-agents(7), sniplib(5)
