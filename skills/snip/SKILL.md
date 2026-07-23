---
name: snip
description: Operate a snip snippet library from the command line — the filesystem-native code-snippet manager (`*.sniplib` directories) built to replace SnippetsLab. Use this skill whenever the user wants to save, find, read, edit, tag, move, or delete code snippets, mentions `snip`, a `.sniplib` library, `snippet.toml`, or "my snippet library", or asks you to pull a saved command, config, or script out of their collection — even when they never name the tool. It covers snip's vocabulary (snippets, fragments, notes, folders, tags, fingerprints), its JSON contract, and the optimistic-concurrency workflow required to edit without destroying concurrent work.
---

# Using the snip CLI

snip stores code snippets as plain files. A library is a directory named
`*.sniplib`; every snippet inside it is a directory ("package") holding a TOML
manifest plus its content files. There is no database — the filesystem is the
source of truth, which is why a human editing in the TUI, an editor writing to
disk, and you running commands can all touch the same library at once.

That concurrency is the single most important thing to internalize. snip gives
you a fingerprint-based guard against it, and this skill is mostly about using
that guard correctly.

## Three habits

**Ask for JSON.** Pass `--output json` (or `jsonl`) to anything you intend to
parse. The human format is aligned for eyes, not parsers, and its columns are
not a stable contract. JSON payload shapes are.

**Never run `snip tui`, and never run a bare `snip`.** The TUI is the
interactive interface for humans. It refuses to start without a terminal
(`usage_error`, exit 2), so it will not hang you — but you also gain nothing
from trying. Everything the TUI does has a CLI equivalent.

**Carry `--if-hash` on every write to something that already exists.** Details
below; this is the part that prevents you from silently overwriting a change
someone made while you were thinking.

## Vocabulary

Use these words when you talk to the user about their library — they match what
the CLI prints and what the TUI shows.

| Term | Meaning |
|---|---|
| **library** | A `*.sniplib` directory. Everything lives under it. |
| **snippet** | One entry: a title, tags, an optional README, and one or more fragments. Stored as a package directory. |
| **package** | The snippet's directory on disk, e.g. `snippets/Scripts/Greeter--79d92dea`. The `--79d92dea` suffix is a UUID prefix; the directory name is cosmetic, the UUID in the manifest is the identity. |
| **fragment** | One content file inside a snippet — a snippet holds several when the same idea has variants (bash + python, or setup + teardown). Ordered, 1-based. |
| **note** | Markdown attached to *one fragment*, explaining that fragment. |
| **README** | Markdown attached to the *whole snippet*. |
| **folder** | A real directory under `snippets/`. Nested paths use `/`, e.g. `Data Science/Queries`. |
| **Uncategorized** | The label for a snippet at the library root with no folder. Its actual folder value is the empty string — that is what you pass to `--folder`. |
| **tag** | A free-form label. Tags live in `tags.toml` and survive even when no snippet uses them. |
| **trash** | Soft-deleted snippets under `trash/`, each with an `entry_id`. Restorable until purged. |
| **fingerprint** | A BLAKE3 hash over the snippet's manifest, README, fragments, and notes. It changes whenever anything in the snippet changes, and it is computed on read, never stored. This is the concurrency token. |
| **pinned / locked** | Pinned sorts first. Locked refuses mutation until unlocked — respect it rather than forcing past it. |

Two naming traps worth knowing: `Fragment` is the *default fragment title*, so
untitled fragments genuinely display as "Fragment", and the word "snippet"
refers to the whole entry, never to a single file.

## Choosing the library

Resolution order: `--library <path>` → `$SNIP_LIBRARY` → the nearest ancestor
directory containing `snip.toml` → `default_library` from the user config.

If the user has one library configured, plain commands just work. When you are
scripting several commands, exporting `SNIP_LIBRARY` once is cleaner than
repeating `--library`.

## Addressing a snippet

Most commands take a `<SELECTOR>`, resolved in this order:

1. **Package path** relative to `snippets/` — `Scripts/Greeter--79d92dea`
2. **UUID prefix**, at least 8 hex digits — `79d92dea`
3. **Exact title** — `Greeter` (exact, case-sensitive, no fuzzy matching)

Prefer the UUID. Titles are ambiguous — two snippets can share one, and an
ambiguous selector is a hard error rather than a guess. When you have just
listed or searched, you already hold the `id`; use it.

Fragments are selected with `--fragment`, taking either a **1-based index**
(`--fragment 2`) or a fragment UUID prefix of 8+ hex digits. Index 0 is an
error, because fragment numbering starts at 1 everywhere in snip.

## Reading

```bash
snip list --output json                       # all snippets, no content
snip list --sort modified --output json       # manual | title | modified | created
snip list --folder Code --output json         # Code and everything under it
snip search "docker compose" --output json    # titles, tags, notes, and content
snip show <selector> --output json            # one snippet, everything, with content
snip cat <selector> --fragment 2              # raw fragment bytes, no decoration
snip path <selector> --fragment 1             # absolute path to a managed file
```

`search` is a substring match with weighted scoring (title beats tags beats
content) and returns per-line excerpts, which makes it the right first move when
the user describes a snippet vaguely. `cat` is the right move when you want to
pipe content somewhere — it emits nothing but the fragment.

`list` and `search` both take `--folder` and `--tag`. **`--folder Code` includes
`Code/Rust` and everything else beneath it**, because that is how a folder reads
to a person and what the TUI sidebar shows. Add `--no-subfolders` for the
narrower "directly in this folder" question, and pass `--folder ""` for
Uncategorized. Matching is case-insensitive, but only on whole path components —
`--folder Cod` does not match `Code`.

`snip cat` and `snip path` are also how you hand a snippet to another tool
without parsing JSON at all.

## Writing safely

Text comes in two forms, and the same pair exists for `--note` and `--readme`:

- `--content '…'` — inline. Reach for this by default; it keeps a write to one
  command with no plumbing.
- `--content-file <PATH>` — from a file, where `-` means stdin. Use it for
  content you already have in a file, or output you are piping from another
  command.

They are mutually exclusive, so passing both is a usage error rather than a
silent choice.

**Creating** is unconditional — nothing exists yet to conflict with:

```bash
snip create \
  --title "Greeter" \
  --folder Scripts \
  --tag demo \
  --language bash \
  --content 'echo hello' \
  --output json
```

**Modifying anything that already exists** is a read-modify-write, and each step
matters:

```bash
# 1. Read the current fingerprint
HASH=$(snip show Greeter --output json | jq -r .fingerprint)

# 2. Write, asserting that nothing changed in between
snip edit Greeter --content 'echo hello world' --if-hash "$HASH" --output json
```

If the snippet changed after step 1, step 2 fails with `conflict` (exit 4)
instead of overwriting. That is the outcome you want: re-read, decide whether
your change still applies, and try again. `--force` skips the check — reach for
it only when the user has told you to overwrite, because it discards whatever
the other writer did.

Every successful mutation returns a `changes` object containing
`old_fingerprint` and `new_fingerprint`. Keep the new one if you are about to
make a second edit; it saves a re-read.

### What `edit` changes

`snip edit <selector>` takes structured flags: `--title`, `--folder`, `--tag`
(repeatable), `--clear-tags`, `--pin`/`--unpin`, `--lock`/`--unlock`,
`--content`/`--content-file`, `--note`/`--note-file`/`--clear-note`,
`--readme`/`--readme-file`/`--clear-readme`, `--language`, `--fragment-title`,
and `--fragment` to say which fragment the content/note flags apply to.

A bare `snip edit <selector>` with no structured flag means "open this in
`$EDITOR`", which is a human gesture. Without a terminal it exits immediately
with a `usage_error` telling you to pass a structured change, so it cannot hang
you — but it also gets nothing done. Always pass at least one structured flag.
The same holds for `--metadata-editor`, `--readme-editor`, and `--note-editor`.

### Multiple fragments

```bash
snip fragment add Greeter --title "Python variant" \
  --language python --content 'print("hi")' --if-hash "$HASH"
snip fragment edit <selector> <fragment> --content '…' --if-hash "$HASH"
snip fragment remove <selector> <fragment> --if-hash "$HASH"
snip fragment reorder <selector> <fragment> --position 1 --if-hash "$HASH"
```

Reach for a new fragment rather than a new snippet when the content is another
take on the same idea — that is what fragments are for, and it keeps the user's
list short.

## Organizing

```bash
snip edit <selector> --folder "Scripts/Shell"   # move a snippet ("" = Uncategorized)
snip folder create "Scripts/Shell"
snip folder rename "Scripts/Shell" "Bash"       # new name is ONE path component
snip folder move "Scripts/Shell" "Archive/Shell" # full destination path
snip folder delete "Scripts/Shell"              # must already be empty
snip tag rename old new                          # across every snippet
snip tag delete obsolete                         # removes it everywhere
```

`folder rename` and `folder move` are different operations: rename keeps the
parent and takes a bare name, move takes a full path and reparents. Passing a
path to `rename` is a usage error rather than a silent move.

## Deleting

```bash
snip delete <selector> --if-hash "$HASH"   # → trash, reversible
snip trash --output json                   # lists entries with entry_id
snip restore <entry_id> [--folder <dest>]
snip purge <entry_id> --yes                # permanent
```

`delete` is a soft delete, so it is a reasonable thing to do on the user's
behalf when they ask. `purge` is not reversible and deliberately requires
`--yes`; confirm with the user before running it rather than adding the flag to
get past the error.

## Errors

Errors print one JSON object under `--output json` and set a distinct exit code,
so you can branch on either:

```json
{"error":{"code":"conflict","message":"snippet changed since it was read: expected 4726…, found a30c…"}}
```

| Code | Exit | Meaning and what to do |
|---|---|---|
| `io_error` | 1 | Filesystem problem. Check the path and permissions. |
| `usage_error` | 2 | Bad arguments or a missing confirmation flag. Fix the command; do not retry unchanged. |
| `not_found` | 3 | Selector matched nothing. List or search to find the real one. |
| `conflict` | 4 | Something changed under you, or a selector was ambiguous, or the snippet is locked. Re-read and reassess — do not reflexively add `--force`. |
| `validation_error` | 5 | The library or input violates the format. `snip doctor` explains it. |

## Health

```bash
snip doctor --output json          # validate; ok:true when clean
snip doctor --repair               # finish or roll back interrupted writes
snip organize --dry-run            # preview package-directory renames
```

Mutations are transactional: snip stages a new package and swaps it in. If a
process dies mid-write, `doctor --repair` resolves the leftover. Run `doctor`
first when a library behaves strangely.

## Reference material

- `references/commands.md` — every command and flag, with the JSON payload
  shapes for list, show, search, mutations, and trash. Read it when you need a
  flag this page did not mention or need to know a field name before parsing.
- `references/data-model.md` — the on-disk layout, what a fingerprint covers,
  and the rules for touching files directly instead of going through the CLI.
  Read it when the user wants bulk/scripted changes, is migrating data, or asks
  what a file in the library is.
