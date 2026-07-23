# snip command reference

Full flag list and JSON payload shapes. `SKILL.md` covers the workflows; this
file is for looking up a specific flag or field name.

## Contents

- [Global options](#global-options)
- [Reading](#reading) — `list`, `search`, `show`, `cat`, `preview`, `path`, `info`
- [Writing](#writing) — `create`, `edit`, `fragment`
- [Organizing](#organizing) — `folder`, `tag`
- [Trash](#trash) — `delete`, `trash`, `restore`, `purge`
- [Maintenance](#maintenance) — `doctor`, `organize`, `init`, `import`, `git`
- [Other](#other) — `open`, `config`, `completion`, `tui`
- [JSON payload shapes](#json-payload-shapes)

## Global options

Accepted by every command:

| Flag | Notes |
|---|---|
| `--library <PATH>` | Overrides discovery. Also readable from `$SNIP_LIBRARY`. |
| `--output human\|json\|jsonl` | `jsonl` emits one object per line — good for streaming large lists. |
| `--color auto\|always\|never` | Only affects terminal preview rendering. |

Optimistic-concurrency flags, accepted by every mutating command:
`--if-hash <FINGERPRINT>` asserts the snippet is unchanged; `--force` skips the
assertion.

## Reading

### `snip list`
`--folder <FOLDER>` `--no-subfolders` `--tag <TAG>` `--sort manual|title|modified|created`

Returns an array of snippet summaries (no content). Pinned snippets sort first
in every mode.

### `snip search <QUERY>`
`--folder <FOLDER>` `--no-subfolders` `--tag <TAG>`

Case-insensitive substring search over titles, tags, notes, README, and fragment
content. Scored: exact title 100, title substring 80, tag 65, README line 50,
note line 45, content line 40. Results are sorted by score and include a `line`
and `excerpt` per match, so one snippet can appear several times.

### Folder filtering (`list` and `search`)

`--folder` selects a folder **and its descendants**, matching the TUI sidebar, so
`--folder Code` returns `Code/Rust` snippets too. `--no-subfolders` narrows it to
that folder alone and is rejected without `--folder`. Comparison is
case-insensitive and component-wise: `--folder Cod` matches nothing, and
`--folder ""` is the library root (Uncategorized) and never expands to the whole
library.

### `snip show <SELECTOR>`
Complete snippet including every fragment's content and the fingerprint.

### `snip cat <SELECTOR>`
`--fragment <INDEX|UUID_PREFIX>`

Raw fragment bytes, no headers. Defaults to the first fragment. This is the
correct way to pipe snippet content into another program.

### `snip preview <SELECTOR>`
`--render ansi|plain|html` `--pager` `--no-pager`

Human-facing rendering with syntax highlighting and rendered Markdown. Use
`--render plain` if you need to read it yourself; prefer `show`/`cat` for
anything you parse.

### `snip path <SELECTOR>`
`--fragment <F>` | `--readme` | `--metadata`

Prints one absolute path. Default is the package directory. `--metadata` is
`snippet.toml`.

### `snip info`
Library metadata plus counts of snippets, fragments, folders, tags, and trash.

## Writing

### `snip create`
Required: `--title <TITLE>`.

| Flag | Notes |
|---|---|
| `--folder <FOLDER>` | Omit or pass `""` for Uncategorized. Created if missing. |
| `--tag <TAG>` | Repeatable. |
| `--language <LANGUAGE>` | Drives the file extension and highlighting. |
| `--fragment-title <TITLE>` | Default is the literal string `Fragment`. |
| `--content <TEXT>` | Inline fragment content. |
| `--content-file <PATH>` | Same, read from a file; `-` reads stdin. Conflicts with `--content`. |
| `--note <TEXT>` / `--note-file <PATH>` | Markdown note for the first fragment. |
| `--readme <TEXT>` / `--readme-file <PATH>` | Markdown README for the snippet. |
| `--pin` / `--lock` | Initial state. |

Defaults for language, folder, and tags come from the user config when the flag
is omitted.

### `snip edit <SELECTOR>`

Structured changes: `--title`, `--folder`, `--tag` (repeatable), `--clear-tags`,
`--pin`, `--unpin`, `--lock`, `--unlock`, `--language`, `--fragment-title`,
`--content` / `--content-file`, `--note` / `--note-file` / `--clear-note`,
`--readme` / `--readme-file` / `--clear-readme`, and `--fragment <F>` to target
one fragment.

External-editor modes spawn `$EDITOR` and block until it exits, so they are for
humans: a bare `edit` with no structured flag, plus `--metadata-editor`,
`--readme-editor`, and `--note-editor`. Each checks for a terminal first and
exits with `usage_error` when there is none, so they fail fast instead of
blocking — but pass a structured flag to actually get work done.

### `snip fragment <SUBCOMMAND>`

| Subcommand | Signature |
|---|---|
| `add <SELECTOR>` | `--title <TITLE>` (required) `--language` `--content`/`--content-file` `--note`/`--note-file` |
| `edit <SELECTOR> <FRAGMENT>` | `--title` `--language` `--content`/`--content-file` `--note`/`--note-file` `--clear-note` |
| `remove <SELECTOR> <FRAGMENT>` | — |
| `reorder <SELECTOR> <FRAGMENT>` | `--position <N>` (1-based) |

`<FRAGMENT>` is a 1-based index or a UUID prefix of 8+ hex digits.

## Organizing

### `snip folder <SUBCOMMAND>`

| Subcommand | Signature | Notes |
|---|---|---|
| `list` | — | Every folder path. |
| `create <FOLDER>` | — | Parents created as needed. |
| `rename <FOLDER> <NEW_NAME>` | | `NEW_NAME` must be a single path component. |
| `move <FOLDER> <TARGET>` | | Full destination path; reparents. Fails if the target exists. |
| `delete <FOLDER>` | — | Must already be empty. |

### `snip tag <SUBCOMMAND>`

`list`, `rename <OLD> <NEW>`, `delete <TAG>`. Rename and delete apply across
every snippet and report how many were touched. Tags persist in `tags.toml`
after their last use, so a tag with count 0 is normal, not corruption.

## Trash

| Command | Notes |
|---|---|
| `delete <SELECTOR>` | Moves to trash. Accepts `--if-hash` / `--force`. |
| `trash` | Lists entries with `entry_id`, `title`, `original_path`, `deleted_at`. |
| `restore <ENTRY_ID>` | `--folder <DEST>` restores elsewhere. Fails with `conflict` if the original path is occupied. |
| `purge <ENTRY_ID>` | Permanent. Requires `--yes`. |

## Maintenance

- `snip doctor [--repair]` — validate; `--repair` finishes or rolls back
  interrupted transactions. Returns `checked`, `errors`, `warnings`,
  `pending_transactions`, `repaired`, `ok`.
- `snip organize [--dry-run]` — normalize package directory names after titles
  change. Cosmetic; identity lives in the UUID.
- `snip init [PATH] [--name <NAME>] [--git]` — create a library. `PATH`
  defaults to the current directory; `--git` initializes a repository in it.
- `snip import snippetslab <SOURCE> --into <LIBRARY> [--dry-run]` — import a
  SnippetsLab database. Always dry-run first and show the user the report, which
  counts snippets, folders, tags, fragments, notes, and attachments and flags
  normalized tags.
- `snip git status|diff|log|commit` — Git scoped to the library directory, for
  libraries kept under version control.

## Other

- `snip open <SELECTOR>` — hand a managed path to an application. Same target
  flags as `path` (`--fragment`, `--readme`, `--metadata`) plus `--app <CMD>`;
  defaults to the `vscode_cmd` config key, then `code`. This launches a GUI
  program, so only run it when the user asked to open something.
- `snip config path|show|init|set <KEY> <VALUE>|unset <KEY>` — keys:
  `default-library`, `output`, `color`, `preview-render`, `preview-pager`,
  `editor`, `pager`, `default-language`, `default-folder`, `default-tags`,
  `tui-theme`, `tui-sort`, `tui-icons`. Unknown keys in the file are preserved
  across writes, so hand-added settings survive.
- `snip completion bash|zsh|fish` — shell completion script.
- `snip tui` — interactive TUI for humans. Refuses to start without a terminal.

## JSON payload shapes

### List entry (`list`)
```json
{
  "id": "79d92dea-277b-453f-86e5-2f2fbbfd0f06",
  "title": "Greeter",
  "folder": "Scripts",
  "tags": ["demo"],
  "fragments": 1,
  "pinned": false,
  "locked": false,
  "created_at": "2026-07-23T21:17:18.899871Z",
  "modified_at": "2026-07-23T21:17:18.900428Z",
  "fingerprint": "472697ff761e33cf…",
  "path": "/…/Skill.sniplib/snippets/Scripts/Greeter--79d92dea"
}
```
`folder` is `""` for Uncategorized. `fragments` is a count here; in `show` it is
an array.

### Search result (`search`)
```json
{
  "snippet_id": "79d92dea-…", "title": "Greeter", "folder": "Scripts",
  "fragment_id": "813f6ac6-…", "fragment_title": "Fragment",
  "line": 1, "excerpt": "echo hello world", "score": 40
}
```
`line` and `fragment_id` are null for title- and tag-only matches.

### Mutation result (`create`, `edit`, `fragment *`, `delete`)
```json
{
  "snippet": { "…full snippet…" },
  "changes": {
    "fields": ["fragments[1].content"],
    "old_fingerprint": "472697ff…", "new_fingerprint": "a30cf13a…",
    "old_path": "/…/Greeter--79d92dea", "new_path": "/…/Greeter--79d92dea"
  }
}
```
`changes` is null on create. `new_path` differs from `old_path` when the title
or folder changed. Carry `new_fingerprint` into your next `--if-hash`.

### Snippet (`show`, and the `snippet` field above)
Adds to the list entry: `readme` (string or absent), `package_path`, and
`loaded_fragments` — an array of `{id, title, language, file, content,
note_content, absolute_path}`. `file` is package-relative; `absolute_path` is
what you hand to other tools.

### Trash entry (`trash`, `delete`)
```json
{
  "entry_id": "4401c19cfc424373a6647224a2bc4553",
  "snippet_id": "79d92dea-…", "title": "Greeter",
  "original_path": "snippets/Scripts/Greeter--79d92dea",
  "deleted_at": "2026-07-23T21:17:48.010056Z",
  "package_path": "/…/trash/20260723211748-Greeter-4401c19c…/package"
}
```
`restore` and `purge` take `entry_id`, not `snippet_id`.

### Error (any command)
```json
{"error": {"code": "conflict", "message": "snippet changed since it was read: expected 4726…, found a30c…"}}
```
Codes: `io_error` (1), `usage_error` (2), `not_found` (3), `conflict` (4),
`validation_error` (5); the number is the process exit code.
