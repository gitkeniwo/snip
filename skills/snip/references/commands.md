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
- [Publishing](#publishing) — `gist`
- [Other](#other) — `open`, `config`, `keys`, `completion`, `tui`
- [JSON payload shapes](#json-payload-shapes)

## Global options

Accepted by every command:

| Flag | Notes |
|---|---|
| `--library <PATH>` | Overrides discovery. Also readable from `$SNIP_LIBRARY`. |
| `--output human\|json\|jsonl` | `jsonl` emits one object per line — good for streaming large lists. |
| `--color auto\|always\|never` | Only affects terminal preview rendering. |
| `--simplified-ui[=<BOOL>]` | Use square TUI bar caps without a Powerline font; bare means `true`. |

Optimistic-concurrency flags, accepted by every mutating command:
`--if-hash <FINGERPRINT>` asserts the snippet is unchanged; `--force` skips the
assertion.

## Reading

### `snip list`
`--folder <FOLDER>` `--no-subfolders` `--tag <TAG>` `--sort manual|title|modified|created`

Returns an array of snippet summaries (no content). Pinned snippets sort first
in every mode.

### `snip search <QUERY>`
`--regex` `--field <FIELD>` `--context/-C <N>` `--limit/-m <N>` `--folder <FOLDER>`
`--no-subfolders` `--tag <TAG>`

Case-insensitive search over titles, tags, README, fragment content, and notes.
Scored: exact title 100, title substring 80, tag 65, README line 50, note line
45, content line 40. Results sort by score and carry a `line` and `excerpt` per
match, so one snippet can appear several times — cap it with `--limit`.

| Flag | Notes |
|---|---|
| `--regex` | QUERY becomes a regular expression (the `regex` crate — linear time, no backreferences). Case-insensitive; `(?-i)` opts out. A bad pattern is a `usage_error`. |
| `--field <FIELD>` | `title`, `tag`, `readme`, `content`, `note`. Repeatable; omitting it searches all five. |
| `--context <N>`, `-C <N>` | Fill `context_before` / `context_after` with N lines each. Human output marks context with `-` and the match with `:`, like ripgrep. |
| `--limit <N>`, `-m <N>` | Keep the top N rows after scoring. |

Every result reports the `field` it matched and the snippet's `fingerprint`.

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
anything you parse. Highlighting uses the expanded two-face syntax set; the TUI
language picker recognizes canonical names, aliases, and extensions while still
accepting custom language values.

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
- `snip init [PATH] [--name <NAME>] [--git] [--yes]` — create a library. In an
  interactive terminal, omitting `PATH` starts setup; `--yes` suppresses setup and
  defaults `PATH` to the current directory. `--git` initializes a dedicated
  repository.
- `snip import snippetslab <SOURCE> --into <LIBRARY> [--dry-run]` — import a
  SnippetsLab database. Always dry-run first and show the user the report, which
  counts snippets, folders, tags, fragments, notes, and attachments and flags
  normalized tags.
- `snip git status` — Git backup status scoped to the library directory.
- `snip git init` — make the library a Git repository when it is not one
  already. Idempotent: on a library that is already inside a repository it
  reports `created: false` and succeeds rather than failing, so it is safe to
  run unconditionally. Creates no commit.
- `snip git commit [-m <MESSAGE>]` — stage and commit library content. The
  generated message includes local time and the dirty-file count.
- `snip git backup` — commit when the library is dirty, then push whenever
  local commits are ahead of the configured upstream. A clean-but-ahead
  library is pushed without creating another commit. It succeeds as an
  idempotent no-op when the library is already up to date. In JSON, `message`
  is emitted only when this invocation created a commit.
- `snip git push` — push ahead commits without committing, for example to retry
  after resolving a rejected push.
- `snip git fetch` — fetch and prune remote-tracking refs without merging,
  switching branches, or changing worktree files. Git mutation commands never
  pull.

CLI Git writes are deliberate user-triggered operations: they disable
credential prompts and fail fast. The TUI runs manual operations on a
background thread without suspending, and suspends to the real terminal only
for `backup_on_quit`, where interactive credentials and progress still matter.
With `[git].auto_commit_interval` set above zero, the TUI may create
local interval commits. `[git].auto_push = true` also pushes ahead commits in a
non-prompting background worker on that same interval. An interval of `0`
disables both. In the TUI Git console, `f` fetches remote status, `i` changes
the interval, `u` toggles auto-push, `o` toggles backup-on-quit, and `a` pauses
automatic commits and pushes for the current session.

## Publishing

`snip gist` publishes a snippet to GitHub Gists through the `gh` CLI; nothing
reaches the network unless `gh` is installed and authenticated. The gist record
lives in the snippet's `snippet.toml` under `[[remotes]]` (see
[data-model.md](data-model.md)), so the link travels with the library. Unlinked-target commands fail with `not_found` (3); `attach`
on a linked snippet is a `conflict` (4); an un-publishable payload is a
`validation_error` (5).

- `snip gist push <SELECTOR> [--public] [--desc <TEXT>] [--new]
  [--include-notes] [--no-readme] [--web] [--if-hash <HASH>] [--force]` —
  upsert. Creates the gist the first time and updates it afterwards, keeping the
  same URL. The description defaults to the snippet title, then to the recorded
  description. `--new` publishes a fresh gist and re-records the link, leaving
  the old gist on GitHub. When nothing changed since the last push it prints
  `gist is already up to date` and makes no network call. `--public` on an
  existing secret gist is a `validation_error`.
- `snip gist url <SELECTOR> [--copy]` — print the gist URL and nothing else;
  `--copy` also copies it. Fails `not_found` (3) when the snippet has no gist.
- `snip gist status [<SELECTOR>|--all] [--remote]` — compare the snippet against
  the last push without touching the network: `clean`, `modified`, or `unlinked`.
  `--remote` also fetches the gist from GitHub and reports `missing` when it no
  longer exists. Requires a selector or `--all`.
- `snip gist attach <SELECTOR> <GIST>` — record an existing gist (ID or URL) as
  this snippet's gist, adopting metadata from GitHub. Fails `conflict` (4) when
  already linked.
- `snip gist detach <SELECTOR>` — forget the gist record; the gist stays on
  GitHub.
- `snip gist delete <SELECTOR> [--yes]` — delete the gist on GitHub and forget
  it. Without `--yes` it prompts on stderr and aborts with `cancelled` on
  anything but `y`; under `--output json` `--yes` is required.
- `snip gist open <SELECTOR>` — open the gist in a browser. Prints nothing.

## Other

- `snip open <SELECTOR>` — hand a managed path to an application. Same target
  flags as `path` (`--fragment`, `--readme`, `--metadata`) plus `--app <CMD>`;
  defaults to the `vscode_cmd` config key, then `code`. This launches a GUI
  program, so only run it when the user asked to open something.
- `snip config path|show|init|set <KEY> <VALUE>|unset <KEY>` — keys:
  `default-library`, `output`, `color`, `preview-render`, `preview-pager`,
  `editor`, `pager`, `default-language`, `default-folder`, `default-tags`,
  `tui-theme`, `tui-sort`, `tui-density`, `tui-line-numbers`,
  `tui-simplified-ui`,
  `git-auto-commit-interval`,
  `git-auto-push`, `git-backup-on-quit`. `tui-line-numbers` is boolean and
  defaults to on. The interval is a whole number of
  minutes, with `0` meaning all automatic Git work is off; both remaining Git
  settings are boolean. Enabling auto-push while the interval is `0` prints a
  warning. Unknown keys in the file are preserved across writes, so hand-added
  settings survive.
- `snip keys list|show <ACTION>|path|export|check` — inspect the TUI key map.
  Bindings live in `keys.toml` beside `config.toml` and are never rewritten by
  `snip config set`. `list` and `show` support `--output json|jsonl`; `export`
  writes an authoritative default file and `check` reports conflicts and
  lockouts. These only describe the interactive TUI, so they are useful for
  answering a user's "what key does X" question, not for doing work.
- `snip completion bash|zsh|fish` — shell completion script.
- `snip tui [--simplified-ui[=<BOOL>]]` — interactive TUI for humans. Refuses
  to start without a terminal. **Toggle Simplified UI** in the command palette
  previews and saves the same preference while the TUI is open.

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
  "fingerprint": "472697ff…", "field": "content",
  "fragment_id": "813f6ac6-…", "fragment_title": "Fragment",
  "line": 4, "excerpt": "kubectl rollout status deploy/api",
  "context_before": ["# roll out", "kubectl apply -f deploy.yaml"],
  "context_after": ["echo done"],
  "score": 40
}
```
`line` and `fragment_id` are null for title- and tag-only matches. `field` is one
of `title`, `tag`, `readme`, `content`, `note`. The context arrays are omitted
entirely unless `--context` was passed. `fingerprint` is the snippet's hash at
scan time — enough to drive `--if-hash` for a metadata change without a second
read, though replacing content still means reading the content first.

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

### Gist push (`push`)
```json
{
  "action": "created",
  "snippet": { "id": "…", "title": "Brewfile", "folder": "Scripts" },
  "gist": {
    "kind": "gist", "host": "github.com",
    "id": "5b0e0062eb8e9654adad7bb1d81cc75f",
    "url": "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f",
    "public": false, "description": "Brewfile",
    "files": ["001-Brewfile", "README.md"],
    "pushed_at": "2026-08-01T10:00:00Z", "pushed_digest": "…"
  },
  "fingerprint": "…"
}
```
`action` is `created`, `updated`, or `unchanged`. `fingerprint` is the snippet's
fingerprint *after* the record was written — carry it into a following
`--if-hash`.

### Gist URL (`url`)
```json
{ "url": "…", "id": "…", "host": "github.com" }
```

### Gist status (`status`)
```json
{
  "state": "modified",
  "snippet": { "id": "…", "title": "Brewfile", "folder": "Scripts" },
  "gist": { "…" }
}
```
`state` is `unlinked`, `clean`, `modified`, or `missing`; `gist` is `null` when
`state` is `unlinked`. With `--all`, an array; under `jsonl`, one object per
line.

### Gist mutation (`attach`, `detach`, `delete`)
```json
{ "action": "attached", "snippet": { "…" }, "gist": { "…" } }
```
`action` is `attached`, `detached`, or `deleted`.

### Error (any command)
```json
{"error": {"code": "conflict", "message": "snippet changed since it was read: expected 4726…, found a30c…"}}
```
Codes: `io_error` (1), `usage_error` (2), `not_found` (3), `conflict` (4),
`validation_error` (5); the number is the process exit code.
