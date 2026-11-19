---
name: snip
description: Operate a snip snippet library from the command line — the filesystem-native code-snippet manager (`*.sniplib` directories) that replaces SnippetsLab. Use whenever the user wants to save, find, read, edit, tag, move, or delete code snippets, mentions `snip`, a `.sniplib` library, `snippet.toml`, or "my snippet library", or asks you to pull a saved command, config, or script from their collection, even without naming the tool. Covers the vocabulary, JSON contract, token-efficient reads, and the `--if-hash` workflow for editing safely alongside a human.
---

# Using the snip CLI

A library is a `*.sniplib` directory; each snippet is a package directory with a
TOML manifest plus content files. No database: a human in the TUI, an editor,
and you can all write at once. The fingerprint guard (`--if-hash`) is what makes
that safe.

## Three habits

1. **Ask for JSON**: `--output json` (or `jsonl`) on anything you parse. Human
   output is not a stable contract.
2. **Never run `snip tui` or bare `snip`.** They are for humans and exit 2
   without a terminal.
3. **Carry `--if-hash` on every write to something that exists.**

## Vocabulary

**snippet** = one entry (title, tags, optional README, fragments). **fragment** =
one content file, 1-based, ordered. **note** = Markdown on one fragment;
**README** = Markdown on the whole snippet. **folder** = real directory
(`Data/Queries`); Uncategorized = `""`. **fingerprint** = BLAKE3 over the whole
snippet, computed on read; the concurrency token. **locked** snippets refuse
writes: respect it. Full table: `references/workflows.md`.

## Library and selectors

Order: `--library` → `$SNIP_LIBRARY` → nearest ancestor `snip.toml` → user
default. If the user names no library, run with none: "my snippets" is the
default. `snip info --output json` shows which one won.

A `<SELECTOR>` is a package path, a UUID prefix (≥8 hex), an exact title, or
`folder/title`. Prefer the UUID you just got from a search. Ambiguous titles are
an error. Fragments: `--fragment N` (1-based) or a UUID prefix.

## Reading (spend bytes on purpose)

```
1 locate   snip search <q> --brief -m 20 --output json
2 outline  snip show <id> --outline --output json
3 read     snip cat <id> --fragment N [--lines A:B] [--max-bytes N]
           snip show <id> --max-bytes N --output json
4 write    snip edit <id> … --if-hash <16-char fingerprint>
```

- `search --brief` → `{total, results[]}`, one row per snippet: `id`, `title`,
  `folder`, `tags`, `languages`, `fragments`, `bytes`, 16-char `fingerprint`,
  `matched_fragments` (`index`, `lines`), `excerpt`. `-m` limits snippets;
  `total` says if the list is complete. Use `search` for lookup, not `rg`.
- `bytes` is the cost of reading the whole snippet. Up to about 8 KB, `show`
  it directly; above that go outline → one fragment → a line range.
- `show --outline` lists fragments (language, bytes, lines, `has_note`) with no
  text. `--max-bytes N` fills README, then each fragment and note in order, and
  cuts on line boundaries.
- `cat --lines A:B` (also `A:`, `:B`, `A`) prints raw bytes; truncation notes
  go to stderr, never into the content.
- **A read with `truncated: true` is not the content.** Read the rest before
  quoting it or editing from it.
- `list --brief` gives compact rows; plain `list`/`search`/`show` keep the full
  shapes. `--folder X` includes subfolders (`--no-subfolders` to stop).
- Search extras: `--regex`, `--field title|tag|readme|content|note`, `-m N`.

## Writing safely

`--content '…'` inline (default) or `--content-file PATH` (`-` = stdin); same for
`--note` and `--readme`. Note = about one file; README = about the set.

```bash
snip create --title Greeter --folder Scripts --tag demo --language bash \
  --content 'echo hello' --output json
HASH=$(snip show Greeter --output json | jq -r .fingerprint)
snip edit Greeter --content 'echo hi' --if-hash "$HASH" --output json
```

- `--if-hash` takes the full fingerprint or any prefix of **16+ hex chars**
  (what brief rows print). Shorter or non-hex → exit 2.
- A stale hash → `conflict` (exit 4): re-read, reassess, retry. `--force`
  only when the user said to overwrite.
- Every mutation returns `changes.new_fingerprint`; reuse it for the next edit.
- A hash from search/list is fine for **metadata** changes (tag, move, rename,
  pin, delete). Before replacing content, read the content first.
- `edit` needs at least one structured flag (`--title --folder --tag
  --clear-tags --pin --lock --content --note --readme --language
  --fragment-title --fragment`). Bare `snip edit` opens `$EDITOR`: a human
  gesture; never use it, nor `--create`.
- More takes on one idea → `snip fragment add|edit|remove|reorder`, each with
  `--if-hash`, not a new snippet.
- **A folder the user names already exists**: `snip folder list --output json`
  and pass the exact path. `--folder` silently creates a new one on a typo.

## More workflows (read `references/workflows.md` when needed)

- Organizing: folder/tag rename, move, delete; matching folder names.
- Deleting: `delete` is soft (trash); `purge --yes` is permanent: ask first.
- Backing up: `snip git status` before bulk or irreversible changes.
- Sharing: `snip gist …`; confirm before `--public`, `--new`, or `delete`.
- Health: `snip doctor`, `doctor --repair`, `organize --dry-run`.

## Errors

JSON errors: `{"error":{"code","message"}}`. `io_error` 1 · `usage_error` 2 (fix
the command) · `no_library`/`not_found` 3 (set the library; list to find the
real selector) · `conflict` 4 (re-read) · `validation_error` 5 (`snip doctor`).

## Reference

`references/commands.md`: every flag and JSON shape. `references/data-model.md`:
on-disk layout and fingerprint rules. `references/workflows.md`: procedures above.
