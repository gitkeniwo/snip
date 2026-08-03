# snip library format v1

This document specifies the on-disk format of a snip library. It exists so the
library is not tied to this implementation: anything described here can be read
and written by other tools, and a library remains usable if snip disappears.

The key words MUST, MUST NOT, SHOULD, and MAY are used in the sense of RFC 2119.
"Reader" means any program that loads a library; "writer" means any program that
modifies one.

This is `schema_version = 1`. Every manifest carries its own `schema_version`.

## Conformance in brief

A reader MUST:

- reject any manifest whose `schema_version` exceeds the version it implements,
  and reject `schema_version = 0`;
- reject symbolic links anywhere in `snippets/`, and reject managed paths that
  resolve outside their package;
- reject non-UTF-8 content in managed files;
- preserve unknown TOML fields through a read-modify-write.

A writer MUST additionally:

- hold the library lock while modifying anything under `snippets/` or `trash/`;
- leave the library in a state a reader accepts, even if it is interrupted.

Unknown-field preservation is what allows different versions and different tools
to share a library without one silently discarding the other's data.

## Library root

A library is a directory, conventionally suffixed `.sniplib`:

```text
Main.sniplib/
├── snip.toml          # library manifest; identifies the root
├── tags.toml          # tag registry
├── snippets/          # folder hierarchy and snippet packages
├── trash/             # soft-deleted packages
├── .snip/             # runtime state; never user data
└── .gitignore         # excludes .snip/ so the library can be versioned
```

`snip.toml` is what identifies a directory as a library root. Readers locate a
library by walking upward from the working directory until they find one.

Libraries are meant to be kept under version control, so `snip init` writes a
`.gitignore` excluding the runtime directories. It is an ordinary file the owner
may edit; readers MUST NOT depend on it.

### What version control carries

| Path | Holds | Durable | Tracked |
|---|---|---|---|
| `snip.toml` | library identity | yes | yes |
| `tags.toml` | tag registry | yes | yes |
| `snippets/` | every snippet package | yes | yes |
| `trash/` | soft-deleted packages | yes | yes |
| `.snip/` | locks, transactions, cache | no | no |
| `.gitignore` | the exclusions above | no | yes |

Durable state is exactly what a clone must reproduce. Everything under `.snip/`
is machine-local coordination state that a reader rebuilds on demand, which is
why excluding it loses nothing.

Nothing outside the library root belongs to the format. A CLI's own settings —
for snip, `$XDG_CONFIG_HOME/snip/config.toml` — describe how one person on one
machine works (which library is the default, editor, pager, colors) and apply
across every library that person opens. A reader MUST NOT require such a file to
be present, and MUST NOT expect one to travel with a library.

### Directories that do not survive a clone

Git records files, not directories, so an empty directory does not reach a
second machine. A library restored from version control therefore arrives
without `.snip/` — always, since all three of its subdirectories are excluded —
and without `snippets/` or `trash/` when either happened to be empty.

Readers MUST treat these four directories as recreatable: create them on open
and continue, rather than rejecting the library. Only `snip.toml` decides
whether a directory is a library root. A reader that cannot create them (a
read-only mount, for instance) MAY refuse to write, but SHOULD still serve reads
it can satisfy.

## `snip.toml`

```toml
format = "snip-library"
schema_version = 1
id = "85f7c597-9c96-41f7-b2a0-b1cab232270b"
name = "Main"
created_at = "2026-07-22T17:00:00Z"
```

| Field | Type | Notes |
|---|---|---|
| `format` | string | MUST be `snip-library`. |
| `schema_version` | integer | MUST be ≥ 1. |
| `id` | UUID | Stable library identity. |
| `name` | string | Display name; carries no meaning. |
| `created_at` | RFC 3339 | |

## `tags.toml`

The registry keeps tag identity and presentation stable, including for tags no
snippet currently uses — so deleting the last snippet with a tag does not lose
the tag's colour or its provenance.

```toml
schema_version = 1

[[tags]]
id = "31296bf7-e575-46c4-a906-f4e22a4019e9"
name = "ffmpeg"
color = 0
source_id = "31296BF7-E575-46C4-A906-F4E22A4019E9"
```

`color` and `source_id` are OPTIONAL; `source_id` records the identifier the tag
had in an imported library.

The registry is a convenience, not an authority: a tag named in `snippet.toml`
is valid whether or not it appears here. Writers SHOULD add newly used tags to
the registry.

## Folders

Folders are ordinary directories under `snippets/`. Their path *is* the folder
path — there is no folder record anywhere. Nesting uses `/`, so
`snippets/Data Science/Queries/` is the folder `Data Science/Queries`. A snippet
directly under `snippets/` has the empty folder path, presented to people as
`Uncategorized`.

A directory containing `snippet.toml` is a snippet package rather than a folder,
and readers MUST NOT descend into it looking for more packages. Packages do not
nest.

An otherwise empty folder is preserved by an empty `.keep` file, because an
empty directory would otherwise be indistinguishable from one never created.
Readers MUST ignore `.keep` when deciding whether a folder is empty; writers
SHOULD create it when a folder becomes empty and remove it when the folder gains
a package.

## Snippet package

Any directory below `snippets/` containing `snippet.toml` is a snippet package.

```text
Brewfile--a5792745/
├── snippet.toml       # manifest: identity, metadata, fragment list
├── README.md          # optional, describes the whole snippet
├── fragments/001-Brewfile
├── notes/001.md       # optional, describes one fragment
└── attachments/       # reserved; see below
```

```toml
schema_version = 1
id = "a5792745-36aa-36ea-9966-f301ff14f3f0"
title = "Brewfile"
tags = ["dotfiles", "homebrew"]
pinned = false
locked = false
created_at = "2026-03-15T10:20:00Z"

[source]
kind = "snippetslab"
library_id = "41B1E541-7610-45CE-A0CF-257C9B5C4682"
original_id = "A5792745-36AA-36EA-9966-F301FF14F3F0"
format_version = "2.6"
modified_at = "2026-03-15T10:25:00Z"

[[fragments]]
id = "f22e0f61-ef44-4021-9380-5ec4842b80b5"
title = "Fragment"
language = "makefile"
file = "fragments/001-Brewfile"
note = "notes/001.md"
source_language = "MakefileLexer"
```

| Field | Notes |
|---|---|
| `id` | The snippet's identity. The directory name is not. |
| `title` | Display title. Need not be unique. |
| `tags` | Trimmed, non-empty, unique under case-insensitive comparison. |
| `pinned`, `locked` | Default `false`. `locked` asks writers to refuse mutation. |
| `created_at` | RFC 3339, authoritative — unlike modification time, which is derived. |
| `source` | OPTIONAL import provenance. `kind` is required within it. |
| `remotes` | OPTIONAL published copies; see [Remotes](#remotes). |
| `fragments` | At least one. Order is presentation order. |

Per fragment: `id` (unique within the snippet), `title`, `language`, `file`, an
OPTIONAL `note`, and an OPTIONAL `source_language` recording the importing
tool's own language name.

`file` and `note` are package-relative paths. They MUST NOT be absolute, MUST
NOT contain `..`, and their canonical form MUST remain inside the package. Every
managed file MUST be valid UTF-8; it MAY be empty.

There is no stored modification time. A snippet's effective modification time is
the newest mtime among its manifest, README, fragment files, and note files.

`attachments/` is created for each package and is reserved. Schema v1 assigns no
meaning to its contents and does not include them in the fingerprint.

## Naming

Writers derive directory and file names from titles for the benefit of people
browsing the library. **Readers MUST NOT infer anything from these names** —
identity lives in the UUIDs, and folder membership lives in the path.

A name component is sanitized by trimming, replacing control characters and
`/`, `\`, `:` with `-` (collapsing runs), truncating to 80 bytes on a character
boundary, then trimming spaces, dots, and dashes. If nothing survives, the
component becomes `untitled`.

| Thing | Pattern | Example |
|---|---|---|
| Package directory | `<title>--<first 8 hex of id>` | `Brewfile--a5792745` |
| Fragment file | `fragments/<NNN>-<title>[.ext]` | `fragments/001-Brewfile` |
| Note file | `notes/<NNN>.md` | `notes/001.md` |

`NNN` is the 1-based position, zero-padded to three digits. The extension comes
from a `language` → extension mapping and is appended only when the sanitized
title has no `.` and is not a name conventionally used without one
(`Brewfile`, `Dockerfile`, `Makefile`, `Justfile`, `Procfile`).

Because names are cosmetic, they can drift from the title after a rename. That
is not corruption, and `snip organize` re-derives them.

## Fingerprint

The fingerprint is a BLAKE3 hash identifying a snippet's exact contents. It is
computed on read and never stored, so it cannot go stale.

It exists for optimistic concurrency: a writer reads a fingerprint, and asserts
it on write (`--if-hash`). If anything in the snippet changed in between, the
write is refused instead of overwriting work the writer never saw. Any tool
implementing this contract MUST compute the hash identically.

Entries are hashed in this order:

1. `snippet.toml` — the raw manifest bytes, under the name `snippet.toml`
2. `README.md` — its bytes, under the name `README.md`, if the file exists
3. For each fragment in manifest order:
   a. its content, under the name given by the fragment's `file` field
   b. its note, under the name given by `note`, if the fragment has one

Each entry is fed to the hasher as four pieces:

```text
<name length as u64 little-endian> <name bytes> <data length as u64 little-endian> <data bytes>
```

Length-prefixing both parts is what makes the hash unambiguous: without it, a
rename could be indistinguishable from an edit. Names are the path strings as
written in the manifest, not resolved paths. `attachments/` is not hashed.

## Remotes

A snippet MAY record where it has been published. Each entry is one
destination; a snippet has at most one entry per `kind`.

```toml
[[remotes]]
kind = "gist"
host = "github.com"
id = "5b0e0062eb8e9654adad7bb1d81cc75f"
url = "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f"
public = false
description = "Brewfile"
files = ["001-Brewfile", "README.md"]
include_notes = false
include_readme = true
pushed_at = "2026-08-01T10:00:00Z"
pushed_digest = "…"
```

| Field | Notes |
|---|---|
| `kind` | Required. `gist` is the only kind schema v1 defines. |
| `host`, `id`, `url` | Required. Identify the published copy. |
| `public` | Whether the remote copy is publicly listed. Defaults to `false`. |
| `description` | The description last sent, which may differ from the title. |
| `files` | The remote filenames this writer published, so a later push knows which to remove. Files not in this list belong to someone else and MUST NOT be deleted. |
| `include_notes` | Whether the payload that produced `pushed_digest` swept in note files. Defaults to `false`. |
| `include_readme` | Whether the payload that produced `pushed_digest` included the readme. Defaults to `true`. |
| `pushed_at` | RFC 3339 timestamp of the last successful publish. |
| `pushed_digest` | Hash of the payload last published. |

`pushed_digest` is deliberately not a snippet fingerprint: recording the entry
changes `snippet.toml`, so a fingerprint captured at publish time would be
stale immediately. It is BLAKE3 over the domain tag `snip-gist-payload-v1`,
then each published `(filename, content)` pair in filename order, then the
description — every piece length-prefixed as a little-endian `u64`, exactly as
in the fingerprint above.

Remotes carry no authority: deleting the entry loses the link, not the snippet.

## Trash

Deletion is reversible. Deleting a snippet moves its package under `trash/`:

```text
trash/20260723211748-Brewfile-4401c19cfc424373a6647224a2bc4553/
├── trash.toml
└── package/           # the snippet package, unchanged
```

```toml
schema_version = 1
entry_id = "4401c19cfc424373a6647224a2bc4553"
deleted_at = "2026-07-23T21:17:48.010056Z"
original_path = "snippets/Scripts/Brewfile--79d92dea"
```

`entry_id` is a UUID in simple (unhyphenated) form and identifies the trash
entry, not the snippet — restore and purge address entries by it. The directory
name embeds the timestamp and title for browsability and is, like other names,
not authoritative.

`original_path` is library-relative. Restoring returns the package there;
if that path is occupied, the restore MUST fail rather than overwrite.

## Runtime state under `.snip/`

`.snip/` holds coordination state, never user data. It MAY be deleted while no
snip process is running, and doing so MUST NOT lose anything from the library.
It MUST NOT be committed to version control.

Because it is never committed, its absence is the normal state of a freshly
cloned library, not a defect. A reader MUST create the directories it needs
instead of treating a missing `.snip/` as a malformed library.

```text
.snip/
├── locks/library.lock
├── transactions/<uuid>/
└── cache/                 # reserved for derived data such as a search index
```

`locks/library.lock` is an advisory exclusive file lock. Writers MUST hold it for
the duration of a mutation; readers do not need it, which is why a scan never
blocks an editor or a TUI.

`cache/` is reserved. Schema v1 stores nothing there, and anything a future
version puts there MUST be derivable from the library itself — the format never
depends on a cache being present or current.

### Transactions

Package mutations are staged, not edited in place, so an interrupted write
cannot leave a half-written snippet. A transaction directory contains
`transaction.toml`, the fully validated staged package, and — during the commit
window — a backup of the previous package.

```toml
schema_version = 1
operation = "replace"
original_path = "…/snippets/Scripts/Brewfile--79d92dea"
target_path = "…/snippets/Scripts/Brewfile--79d92dea"
```

`target_path` differs from `original_path` when the change also renames or moves
the package, such as a retitle or a folder change.

Commit swaps the backup out and the staged package in. Recovery after a crash
prefers a complete committed target; otherwise it restores the backup.
`snip doctor --repair` performs recovery and reports what it did.

## Relationship to the CLI

The format is the contract; the CLI is one implementation of it. `snip doctor`
validates a library against the rules above and is the quickest way to check
whether a hand-edited or externally generated library conforms.
