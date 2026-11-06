%%DESCRIPTION
Read and inspect snippets without modifying them. list returns summaries, search returns scored matching lines, show returns one complete snippet, cat prints raw fragment bytes, preview renders human-facing output, path resolves managed files, open launches an application, and info reports library counts.

A snippet selector is resolved as a package path relative to snippets, a UUID prefix of at least eight hexadecimal digits, an exact case-sensitive title, or a folder and title joined by a slash, split at the final slash and tried only when no title matches the whole selector. An ambiguous selector is reported as not_found and lists the package paths of its candidates. Fragment selectors are a 1-based index or a fragment UUID prefix.

A citation is a snippet UUID prefix followed by # and a fragment selector, such as 937caecb#2 or 937caecb#0a1b2c3d. show opens the whole snippet and marks the cited fragment, adding cited_fragment to its JSON; cat, path, open, and edit act on that fragment, exactly as --fragment would, and giving both is a usage error. Commands that act on a whole snippet reject the #n part. A selector that resolves as written still wins, so titles and paths containing # keep working. The list, show, and search JSON rows carry a citation field holding the shortest prefix, at least eight hexadecimal digits, that is unique among live snippets.

For list and search, --folder includes the named folder and all descendants. Add --no-subfolders for only the direct folder, and pass --folder "" for Uncategorized. Folder matching is case-insensitive on complete path components. --tag restricts results to one tag.

Use show for structured content and fingerprints, cat for pipelines, and preview for people. JSON and JSONL output contracts are documented in snip-agents(7).

When cat is used without --fragment on a snippet containing multiple fragments, it still prints only fragment 1 and writes a selection note to standard error.

%%EXAMPLES
List every snippet below the Code folder carrying the rust tag:

    snip list --folder Code --tag rust

Search content and notes with two lines of context, keeping ten matches:

    snip --output json search deploy --field content --field note --context 2 --limit 10

Print the second fragment exactly as stored:

    snip cat 79d92dea --fragment 2

%%SEE ALSO
snip(1), snip-create(1), snip-edit(1), snip-agents(7), sniplib(5)
