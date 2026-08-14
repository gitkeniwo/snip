%%DESCRIPTION
snip exposes deterministic JSON and JSONL output, stable exit codes, UUID selectors, and optimistic concurrency for scripts and agents. Use structured output for anything parsed; human output is not an API.

%%OUTPUT MODES
--output json emits one JSON value. --output jsonl emits one object per line for streaming lists. Diagnostics go to standard error so standard output remains parseable. Any error under structured output is an object with error.code, error.message, and optional error.hint.

Stable codes are io_error exit 1, usage_error exit 2, no_library or not_found exit 3, conflict exit 4, and validation_error exit 5.

%%SELECTORS
Prefer the UUID returned by list, search, or show. A package path relative to snippets and an exact case-sensitive title are also accepted. UUID prefixes require at least eight hexadecimal digits. Ambiguous titles are conflicts rather than guesses.

Fragments use a 1-based index or fragment UUID prefix. Trash restore and purge use entry_id, not snippet_id.

%%READ CONTRACTS
list emits summaries with id, title, folder, tags, fragment count, pinned, locked, timestamps, fingerprint, and path. folder is an empty string for Uncategorized.

search emits one row per matching line with snippet_id, title, folder, fingerprint, field, optional fragment metadata, line, excerpt, optional context arrays, and score. field is title, tag, readme, content, or note.

show emits a complete snippet, including package_path, optional readme, fingerprint, and loaded_fragments. Each loaded fragment contains id, title, language, relative file, content, optional note_content, and absolute_path. cat is preferable when only raw fragment bytes are needed.

%%WRITE CONTRACT
Mutations emit snippet plus changes. changes contains fields, old_fingerprint, new_fingerprint, old_path, and new_path; create uses null because no prior snippet exists. Carry new_fingerprint into the next write.

Read the current record before replacing content. Assert its fingerprint with --if-hash. A conflict means another writer changed, locked, or ambiguously selected the target; re-read and reassess rather than automatically adding --force.

    record=$(snip --output json show 79d92dea)
    hash=$(printf '%s' "$record" | jq -r .fingerprint)
    snip --output json edit 79d92dea --title "Greeter v2" --if-hash "$hash"

Search results can supply a fingerprint directly for metadata changes. Replacing content still requires reading the content being overwritten.

%%CONTENT INPUT
Content, notes, and READMEs accept inline flags or corresponding --*-file paths. A path of - reads UTF-8 from standard input. The inline and file forms conflict. Commands that would launch an editor require a terminal and fail fast under automation.

    snip edit 79d92dea --content-file - --if-hash "$hash" <<'EOF'
    replacement content
    EOF

%%JSON EXAMPLES
List one folder recursively and stream results:

    snip --output jsonl list --folder Scripts

Search selected fields with bounded output:

    snip --output json search deploy --field title --field tag --limit 10

%%SAFETY
Use --force only with explicit authority to overwrite. delete is reversible; purge and remote gist deletion are not. Public or replacement gist publication changes external state. For bulk direct filesystem writes, ensure no TUI is open and run snip doctor afterward.

Before multi-step writes, retain the newest fingerprint. Treat locked snippets and unknown fields as ownership boundaries. Never parse human tables or scrape package directory names for identity.

%%SEE ALSO
snip(1), snip-query(1), snip-create(1), snip-edit(1), snip-trash(1), sniplib(5)
