%%DESCRIPTION
Read and inspect snippets without modifying them. list returns summaries, search returns scored matching lines, show returns one complete snippet, cat prints raw fragment bytes, preview renders human-facing output, path resolves managed files, open launches an application, and info reports library counts.

A snippet selector is resolved as a package path relative to snippets, a UUID prefix of at least eight hexadecimal digits, an exact case-sensitive title, or a folder and title joined by a slash, split at the final slash and tried only when no title matches the whole selector. An ambiguous selector is reported as not_found and lists the package paths of its candidates. Fragment selectors are a 1-based index or a fragment UUID prefix.

For list and search, --folder includes the named folder and all descendants. Add --no-subfolders for only the direct folder, and pass --folder "" for Uncategorized. Folder matching is case-insensitive on complete path components. --tag restricts results to one tag.

For agents with a limited context budget, three opt-in flags keep reads small. search --brief returns one row per snippet with its size in bytes, short fingerprint, and matched fragments, and no content; list --brief returns compact rows; show --outline returns fragments with language, bytes, and line counts but no text, and show --max-bytes N returns text up to a byte budget. cat accepts --lines A:B and --max-bytes N to print a slice. Cuts always land on a line boundary unless a single line exceeds the budget. cat reports truncation on standard error and keeps standard output as raw bytes. In JSON output, the brief, outline, and max-bytes shapes are compact one-line JSON. Defaults are unchanged without these flags.

Use show for structured content and fingerprints, cat for pipelines, and preview for people. JSON and JSONL output contracts are documented in snip-agents(7).

When cat is used without --fragment on a snippet containing multiple fragments, it still prints only fragment 1 and writes a selection note to standard error.

%%EXAMPLES
List every snippet below the Code folder carrying the rust tag:

    snip list --folder Code --tag rust

Search content and notes with two lines of context, keeping ten matches:

    snip --output json search deploy --field content --field note --context 2 --limit 10

Find snippets without reading content, then print part of one:

    snip --output json search deploy --brief --limit 20
    snip --output json show 79d92dea --outline
    snip cat 79d92dea --fragment 2 --lines 10:40

Print the second fragment exactly as stored:

    snip cat 79d92dea --fragment 2

%%SEE ALSO
snip(1), snip-create(1), snip-edit(1), snip-agents(7), sniplib(5)
