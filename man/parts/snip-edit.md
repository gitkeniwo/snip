%%DESCRIPTION
Modify snippet metadata and content, manage fragments, organize physical folders and tags, and normalize package directory names. Mutations use staged filesystem transactions and advisory locking.

Every read reports a BLAKE3 fingerprint computed from the snippet manifest, README, fragments, and notes. For a safe read-modify-write, read the current fingerprint and pass it back with --if-hash. If another writer changed the snippet, snip exits with conflict instead of overwriting it. Carry new_fingerprint from a successful mutation into the next write.

To change part of a large fragment, pass --lines A:B with --content or --content-file: only those 1-based inclusive lines are replaced, using the same range syntax as snip cat --lines, and an empty replacement deletes them. Writing back exactly what cat --lines printed leaves the fragment unchanged. The write is pinned to the fingerprint read for the splice. With --brief, create, edit, and fragment report the brief snippet entry instead of the full snippet, so a write does not echo its text.

Use --force only when overwriting concurrent changes or bypassing a lock is intentional. Folder rename changes one path component; folder move accepts a full destination path. Fragment positions are 1-based.

The external-editor form accepts --create to create a missing snippet before opening it. It cannot be combined with --if-hash or content and metadata mutation flags; use snip create for non-interactive creation. On a missing selector, --folder, --tag, and --language configure the new snippet. A selector containing a slash uses everything before the final slash as the folder and the final component as the title.

%%EXAMPLES
Rename a snippet with optimistic concurrency:

    hash=$(snip --output json show Greeter | jq -r .fingerprint)
    snip edit Greeter --title "Friendly greeter" --if-hash "$hash"

Create a missing nested snippet and open its first fragment:

    snip edit scratch/testsheet --create

Replace lines 140 to 152 of the first fragment without reading the rest:

    snip cat Greeter --fragment 1 --lines 140:152 > part.txt
    snip edit Greeter --fragment 1 --lines 140:152 --content-file part.txt --if-hash "$hash" --brief

Add a second fragment from a file:

    snip fragment add Greeter --title "Python variant" --language python --content-file greeter.py --if-hash "$hash"

Move a folder to a new parent:

    snip folder move Scripts/Shell Archive/Shell

%%SEE ALSO
snip(1), snip-create(1), snip-query(1), snip-trash(1), sniplib(5), snip-agents(7)
