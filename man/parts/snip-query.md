%%DESCRIPTION
Read and inspect snippets without modifying them. list returns summaries, search returns scored matching lines, show returns one complete snippet, cat prints raw fragment bytes, preview renders human-facing output, path resolves managed files, open launches an application, and info reports library counts.

A snippet selector is resolved as a package path relative to snippets, a UUID prefix of at least eight hexadecimal digits, or an exact case-sensitive title. Ambiguous titles are conflicts. Fragment selectors are a 1-based index or a fragment UUID prefix.

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
