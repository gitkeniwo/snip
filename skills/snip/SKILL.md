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

**When the user does not name a library, do not ask which one and do not go
looking for one.** Run the command with no `--library` and let the order above
land on their default — "my snippets" means the default library. Pass
`--library` only when the user named a specific library or path.

One trap in that order: the nearest-ancestor `snip.toml` outranks
`default_library`, so when your working directory sits inside some other
library, plain commands quietly target *that* one instead of the default.
`snip info` below reports which one won — check it before writing when you might
be inside a library.

When you are scripting several commands, exporting `SNIP_LIBRARY` once is
cleaner than repeating `--library`.

A freshly cloned library works directly with `snip --library <path>`: snip
recreates missing runtime directories when it opens the library, so it does not
need `snip init` first.

`snip info --output json` reports the resolved library and its counts — the
cheapest way to confirm you are pointed where you think you are. `snip init
<path> --name <NAME> [--git] [--yes]` creates a new one, with `--git` making it a
repository from the start. In an interactive terminal, bare `snip` and `snip init`
offer setup; agents must use an explicit path or `--yes` and should never depend on
the prompt.

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

**Search the library with `snip search`, not `rg`.** Both read the same files,
but `rg` returns paths inside package directories that you then have to map back
to snippets, and it cannot see titles or tags at all. `snip search` knows the
structure: it searches titles, tags, README, fragment content, and notes, scores
them (title beats tags beats content), and hands back snippet IDs, folders, and
fingerprints. `cat` is the right move when you want to pipe content somewhere —
it emits nothing but the fragment.

```bash
snip search 'kubectl (apply|rollout)' --regex     # full regex, not just substrings
snip search "rollout" --context 2                 # surrounding lines, like rg -C
snip search "deploy" --field title --field tag    # narrow the domain
snip search "deploy" --limit 10                   # top N after scoring
```

- `--regex` is case-insensitive like the rest of search; put `(?-i)` in the
  pattern to opt out. An unparsable pattern is a `usage_error`, not silence.
- `--context N` fills `context_before` / `context_after` on each result, so you
  can judge a match without a follow-up read. They are omitted when unused.
- `--field` takes `title`, `tag`, `readme`, `content`, or `note`, repeatable.
  Every result also reports the `field` it matched, so hits are self-describing.
- `--limit` matters more than it looks: search emits one row per matching *line*,
  so a common word across a large library returns a lot of rows. Cap it.

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

Choosing between the two prose slots: prose about **this one file** is a note,
prose about **the set** is the README. Watch the scope on `create` — `--note`
attaches to the single fragment it creates, so a description of the whole
snippet written there ends up owned by one file instead of by the snippet.

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

`list` and `search` also carry a `fingerprint`, which lets you go straight from
finding a snippet to changing it. That shortcut is sound for **metadata** —
retag, move, rename, pin, delete — where the change does not depend on the
content. It is not a licence to skip reading before you replace content: a
search result is one matching line, and `--if-hash` only proves nobody else
edited the snippet, not that you knew what you were overwriting. When you are
replacing content you have not read, read it first.

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

### Matching a folder the user named

**Treat a folder the user mentions as one that already exists.** Real folder
names carry capitalization, emoji, and spacing that nobody reproduces from
memory — `AI 👾`, `Data Science/Queries` — so "put it in AI" means that folder,
not a new one.

Resolve the name before you use it: run `snip folder list --output json`, match
what the user said against the real paths while ignoring case, emoji,
punctuation, and small typos, then pass the **exact existing path**. Create a
folder only when the user explicitly asks for a new one, or when nothing in the
list is a plausible match. When the match was not exact, tell the user which
folder you picked.

This matters because `--folder` on `create` and `edit` creates the folder when
it does not exist — silently, exit 0, no warning. snip folds case for you, so
`ai 👾` does find `AI 👾`. **Nothing else is forgiving:** a missing emoji, a
doubled space, or a typo yields a second folder beside the real one, with the
snippet filed in the wrong place.

```bash
snip folder list --output json                 # ["AI 👾"]
snip create --title Probe --folder "AI" …      # exit 0, no warning
snip folder list --output json                 # ["AI", "AI 👾"] — silent duplicate
```

`snip folder delete` cleans that up, but only once the stray folder is empty, so
it is far cheaper to list first than to unpick afterwards.

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

## Backing up

A library can be a Git repository, and `snip git` operates on it scoped to the
library directory:

```bash
snip git status --output json    # ahead/behind, dirty counts, upstream
snip git init                    # make the library a repo; idempotent
snip git commit -m "…"           # stage and commit library content
snip git backup                  # commit if dirty, then push when ahead
snip git push                    # push without committing
snip git fetch                   # refresh remote-tracking refs only
snip git pull [--ff-only]        # fetch and merge the configured upstream
snip git clone <remote> [path]   # restore a library; --gh for private GitHub repos
```

**Before any bulk or irreversible change — a retag across many snippets, a
folder move, a `--force` write, `purge` — run `snip git status`.** It answers
with `available` plus, when true, a `status` object carrying `ahead`, `behind`,
`staged`/`unstaged`/`untracked`, and `state`. A clean repo means the user can
undo you with one `git checkout`; a dirty one means you should say so before
adding your changes to the pile.

An unavailable library is **not an error** — `available: false` with a
`reason.kind` of `not_a_repository`, `binary_missing`, or `probe_failed`, and
exit 0. Branch on `available`, not on the exit code. When it is false the safety
net does not exist; tell the user instead of assuming it does.

Git commands never switch branches and never prompt for credentials — a push or
pull needing a passphrase fails fast instead of hanging you. `snip git pull`
aborts a conflicting merge and leaves the library untouched; use raw Git when
the branches need manual reconciliation. Automatic commits, pushes, and the
optional one-time pull at TUI startup are driven by user config; do not count on
them having run.

`snip git clone` never prompts: a remote that needs credentials fails fast.
Never type a credential yourself — surface the error and its hint to the user.

## Sharing

`snip gist` publishes a snippet to GitHub Gists through `gh`, recording the link
in the snippet's `snippet.toml`. It needs `gh` installed and authenticated with
the `gist` scope; if it is not, say so and let the user run `gh auth login` and
`gh auth refresh -h github.com -s gist` themselves rather than running them for
them.

```bash
snip gist push Brewfile              # create or update the gist
snip gist url Brewfile --copy        # print and copy the link
snip gist status Brewfile            # clean / modified / unlinked
snip gist attach Brewfile <gist-id>  # adopt an existing gist
snip gist detach Brewfile            # forget the link, keep the gist
snip gist delete Brewfile --yes      # delete the gist on GitHub
```

`push` is an upsert: the first run creates the gist, later runs update the same
one and keep the URL, and an unchanged snippet skips the network. Each fragment
becomes one gist file and the README becomes `README.md`.

Gists are secret unless `--public` is passed at creation, and GitHub cannot
change visibility afterwards — `--public` on an existing gist is a
`validation_error`, and the only way round it is `--new`, which publishes a
fresh gist at a new URL. **Publishing is public-facing and irreversible, so
confirm with the user before `--public`, `--new`, or `delete`.**

A `validation_error` means the snippet has an empty fragment that GitHub would
drop; `not_found` (exit 3) on `url`/`detach`/`delete`/`open` means the snippet
has no gist record.

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
| `no_library` | 3 | No library could be resolved. `message` remains a stable machine field; an optional sibling `hint` supplies human next steps. Set `SNIP_LIBRARY`, pass `--library`, or initialize/configure a library. |
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
  It also covers the commands this page leaves out because they are rarely
  yours to run: `import` (SnippetsLab migration), `config`, `keys` (TUI key
  map), `preview`, `open`, and `completion`.
- `references/data-model.md` — the on-disk layout, what a fingerprint covers,
  and the rules for touching files directly instead of going through the CLI.
  Read it when the user wants bulk/scripted changes, is migrating data, or asks
  what a file in the library is.
