# snip library format v1

This document specifies the on-disk format of a snip library. It exists so the
library is not tied to this implementation: anything described here can be read
and written by other tools, and a library remains usable if snip disappears.

The key words MUST, MUST NOT, SHOULD, and MAY are used in the sense of RFC 2119.
"Reader" means any program that loads a library; "writer" means any program that
modifies one.

This is `schema_version = 1`. Every TOML manifest defined by this format carries
its own `schema_version`; nested records such as `source`, `remotes`, and
`fragments` inherit the version of their containing manifest.

## Conformance in brief

A reader MUST:

- reject any manifest whose `schema_version` exceeds the version it implements,
  and reject `schema_version = 0`;
- reject symbolic links anywhere in `snippets/`, including inside a package's
  reserved or otherwise unreferenced directories, and reject managed paths that
  resolve outside their package;
- reject non-UTF-8 content in managed files;
- preserve unknown TOML fields through a read-modify-write.

A writer MUST additionally:

- hold the library lock while modifying anything under `snippets/` or `trash/`;
- leave the library in a state a reader accepts, even if it is interrupted.

Conformance errors do not all have the same operational severity. A reader
cannot open a library whose root manifest is unreadable, unparseable, identifies
another format, or uses an unsupported schema. A scan cannot safely load a
package whose structure is ambiguous or unsafe — for example, an escaping path,
missing managed file, symbolic link, or non-UTF-8 managed content. By contrast,
semantic metadata mistakes such as a malformed timestamp or remote record do not
make the stored snippet content unsafe to read. Readers SHOULD keep such content
available and report the metadata error through an integrity check. Writers MUST
not create new semantic errors.

Unknown-field preservation is what allows different versions and different tools
to share a library without one silently discarding the other's data. It applies
to TOML values, not presentation: a writer MAY reorder fields, remove comments,
or change whitespace when it serializes a manifest again.

### Atomic writes

A writer replacing a single file MUST write a temporary file in the destination
directory, flush its contents, and atomically rename it over the destination.
Where the platform supports it, the writer SHOULD also sync the containing
directory so the rename survives a power loss. A multi-file package change MUST
stage and validate a complete replacement before swapping it into place; the
transaction protocol below is schema v1's recovery mechanism for that swap.

## Library root

A library is a directory, conventionally suffixed `.sniplib`:

```text
Main.sniplib/
├── snip.toml          # library manifest; identifies the root
├── tags.toml          # optional tag registry; missing means empty
├── snippets/          # folder hierarchy and snippet packages
├── trash/             # soft-deleted packages
├── .snip/             # runtime state; never user data
└── .gitignore         # excludes .snip/ so the library can be versioned
```

`snip.toml` is the only file that identifies a directory as a library root.
Discovery policy is an implementation concern rather than part of the format.
The snip CLI resolves an explicit `--library`, then `SNIP_LIBRARY`, then the
nearest ancestor containing `snip.toml`, then its configured default library.

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
| `.snip/` | locks, transactions, cache | no after completed writes | no |
| `.gitignore` | the exclusions above | no | yes |

Durable state is exactly what a clone must reproduce. `.snip/` is machine-local
and is therefore excluded from clones, but an interrupted transaction can
temporarily hold the only recoverable copy of a package. Do not delete pending
transactions; after recovery completes, the runtime tree is rebuildable.

Nothing outside the library root belongs to the format. snip's `config.toml`,
`keys.toml`, `state.toml`, and user theme directory describe how one person on
one machine works and apply across every library that person opens. They
normally live under `$XDG_CONFIG_HOME/snip/`. A reader MUST NOT require any of
them to be present, and MUST NOT expect them to travel with a library.

### Directories that do not survive a clone

Git records files, not directories, so an empty directory does not reach a
second machine. A library restored from version control therefore arrives
without `.snip/` and without `snippets/` or `trash/` when either happened to be
empty.

Readers MUST treat these five concrete directories as recreatable:
`snippets/`, `trash/`, `.snip/cache/`, `.snip/locks/`, and
`.snip/transactions/`. A reader MAY create them on open. An implementation that
supports read-only libraries SHOULD also be able to treat missing empty
directories as empty without creating them; snip currently recreates them on
open and refuses the open if that is impossible.

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
| `name` | string | Writers MUST emit a non-empty trimmed value. Display name; carries no identity. snip strips one trailing `.sniplib` when presenting it. |
| `created_at` | string | Writers MUST emit RFC 3339; snip emits UTC. Readers MAY continue after reporting a malformed legacy value. |

## `tags.toml`

The registry keeps tag identity and presentation stable, including for tags no
snippet currently uses — so deleting the last snippet with a tag does not lose
the tag's colour or its provenance. `tags.toml` is OPTIONAL; a missing file is
an empty registry. Omitting it can therefore discard metadata for unused tags,
but never invalidates tags named by snippets.

```toml
schema_version = 1

[[tags]]
id = "31296bf7-e575-46c4-a906-f4e22a4019e9"
name = "ffmpeg"
color = 0
source_id = "31296BF7-E575-46C4-A906-F4E22A4019E9"
```

The top-level `schema_version` is required. `tags` is OPTIONAL and defaults to
an empty array.

Each entry requires a UUID `id` and a non-empty `name`. Names and UUIDs MUST be
unique within the registry; name comparison uses Unicode lowercase mapping.
`color` is an OPTIONAL signed integer. `source_id` is an OPTIONAL string that
records the identifier the tag had in an imported library.

The registry is a convenience, not an authority: a tag named in `snippet.toml`
is valid whether or not it appears here. Writers SHOULD add newly used tags to
the registry.

## Folders

Folders are ordinary directories under `snippets/`. Their path *is* the folder
path — there is no folder record anywhere. Nesting uses `/`, so
`snippets/Data Science/Queries/` is the folder `Data Science/Queries`. A snippet
directly under `snippets/` has the empty folder path, presented to people as
`Uncategorized`. New folder paths MUST be safe relative paths using `/`
separators, with no empty, `.`, or `..` components. Writers SHOULD also avoid
`\` and `:` so the library remains portable. Readers SHOULD accept an existing
platform-representable folder whose name violates only those portability
recommendations, and integrity checks SHOULD warn about it.

Schema v1 does not make Unicode normalization, case folding, or Windows device
names validity requirements because doing so would invalidate existing Unix
libraries. Tools targeting cross-platform round trips SHOULD normalize new names
to NFC, avoid case-insensitive sibling collisions, and avoid complete path
components named `CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, or `LPT1`–`LPT9`
(with or without an extension). These are portability recommendations, not
reader rejection rules.

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

| Field | Type | Required | Default / validation |
|---|---|---|---|
| `schema_version` | integer | yes | `1` for this specification. |
| `id` | UUID | yes | Globally unique snippet identity; the directory name is not identity. |
| `title` | string | yes | Trimmed value MUST be non-empty; titles need not be unique. |
| `tags` | array of strings | no | `[]`; values are trimmed, non-empty, and unique under Unicode-lowercase comparison. |
| `pinned`, `locked` | boolean | no | `false`; `locked` asks writers to refuse mutation. |
| `created_at` | string | yes | Writers MUST emit RFC 3339; snip emits UTC. Readers MAY continue after reporting a malformed legacy value. |
| `source` | table | no | Import provenance; writers require a non-empty `kind` when present. |
| `remotes` | array of tables | no | `[]`; see [Remotes](#remotes). |
| `fragments` | array of tables | yes | At least one; order is presentation order. |

Within `source`, writers require `kind` to be a non-empty string. `library_id`,
`original_id`, `format_version`, and `modified_at` are OPTIONAL strings;
writers require `modified_at`, when present, to be RFC 3339. Readers SHOULD
report malformed legacy source metadata without hiding the snippet.

Per fragment, `id`, `title`, `language`, and `file` are required. `id` MUST be
unique within the snippet. Trimmed `title` and `language` values MUST be
non-empty. `note` is OPTIONAL. `source_language` is an OPTIONAL string recording
the importing tool's own language name.

`file` and `note` are non-empty package-relative paths. Their stored separator
MUST be `/`; they MUST NOT contain `\`, empty components, `.`, `..`, or `:` and
MUST NOT be absolute. Their resolved form MUST remain inside the package. Every
managed file MUST be a regular UTF-8 file; it MAY be empty.

The managed files are `snippet.toml`, optional `README.md`, and every fragment
or note named by the manifest. Other files are unreferenced extension data:
readers assign them no meaning, writers SHOULD preserve them, and they do not
affect the fingerprint or effective modification time. Binary data is permitted
only in such unreferenced files, conventionally below `attachments/`.

There is no stored modification time. A snippet's effective modification time is
the newest mtime among its manifest, README, fragment files, and note files.

`attachments/` is created by snip for each new package and is reserved. Schema
v1 assigns no meaning to its contents. Readers MUST NOT require it to exist.

## Naming

Writers derive directory and file names from titles for the benefit of people
browsing the library. **Readers MUST NOT infer anything from these names** —
identity lives in the UUIDs, and folder membership lives in the path.

A canonical name component is sanitized by trimming, replacing control
characters and `/`, `\`, `:` with `-` (collapsing runs), truncating to at most
80 UTF-8 bytes on a character boundary, then trimming spaces, dots, and dashes.
If nothing survives, the component becomes `untitled`.

| Thing | Pattern | Example |
|---|---|---|
| Package directory | `<title>--<first 8 hex of id>` | `Brewfile--a5792745` |
| Fragment file | `fragments/<NNN>-<title>[.ext]` | `fragments/001-Brewfile` |
| Note file | `notes/<NNN>.md` | `notes/001.md` |

At creation time, `NNN` is the 1-based position, zero-padded to three digits.
The extension comes from the writer's `language` → extension mapping and is
appended only when the sanitized title has no `.` and is not a name
conventionally used without one (`Brewfile`, `Dockerfile`, `Makefile`,
`Justfile`, `Procfile`). The mapping is not part of conformance because the
manifest path is authoritative.

Because names are cosmetic, they can drift from titles and positions after a
rename, fragment reorder, or external edit. That is not corruption. `snip
organize` re-derives package directory names only; it does not rename fragment
or note files.

## Fingerprint

The fingerprint is a BLAKE3 hash identifying a snippet's exact managed
contents. It is computed on read and never stored, so it cannot go stale. Its
external representation is the 32-byte digest encoded as exactly 64 lowercase
hexadecimal characters.

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
<name byte length as u64 little-endian> <UTF-8 name bytes>
<data byte length as u64 little-endian> <data bytes>
```

Length-prefixing both parts is what makes the hash unambiguous: without it, a
rename could be indistinguishable from an edit. Names are the exact UTF-8 path
strings written in the manifest, not resolved paths. The raw `snippet.toml`
bytes include comments, whitespace, and field ordering, so presentation-only
manifest edits change the fingerprint. No unreferenced file is hashed,
including everything below `attachments/`.

## Remotes

A snippet MAY record where it has been published. Each entry is one
destination. Writers MUST emit at most one entry per `kind`; readers SHOULD
report duplicates or malformed remote metadata but MAY continue to expose the
local snippet.

```toml
[[remotes]]
kind = "gist"
host = "github.com"
id = "5b0e0062eb8e9654adad7bb1d81cc75f"
url = "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f"
public = false
description = "Brewfile"
files = ["Brewfile", "README.md"]
include_notes = false
include_readme = true
```

| Field | Type | Required | Default / meaning |
|---|---|---|---|
| `kind` | string | yes | Non-empty; `gist` is the only kind schema v1 defines. A snippet MUST have at most one record per kind. |
| `host`, `id`, `url` | string | yes | Non-empty identifiers for the published copy. |
| `public` | boolean | no | `false`; whether the remote copy is publicly listed. |
| `description` | string | no | Description last sent, which may differ from the title. |
| `files` | array of strings | no | `[]`; unique non-empty filenames this writer published. Files not in this list belong to someone else and MUST NOT be deleted. |
| `include_notes` | boolean | no | `false`; whether the digested payload included note files. |
| `include_readme` | boolean | no | `true`; whether the digested payload included the README. |
| `pushed_at` | RFC 3339 string | no | Time of the last successful publish. Absent after merely attaching an existing remote. |
| `pushed_digest` | string | no | 64-character lowercase BLAKE3 digest of the last published payload. Absent after attach. |

`pushed_digest` is deliberately not a snippet fingerprint: recording the entry
changes `snippet.toml`, so a fingerprint captured at publish time would be
stale immediately. It is computed as follows, where every string is encoded as
UTF-8 and every length is its byte length:

1. feed `u64_le(20)`, then `snip-gist-payload-v1`;
2. sort published files by filename in ascending UTF-8 byte order;
3. for each file, feed `u64_le(filename length)`, filename bytes,
   `u64_le(content length)`, then content bytes;
4. feed `u64_le(description length)`, then description bytes;
5. encode the 32-byte BLAKE3 digest as 64 lowercase hexadecimal characters.

As an independent fixed test vector, for the description `Brewfile` and files
`001-Brewfile` =
`tap 'homebrew/bundle'\n` and `README.md` = `# Brewfile\n`, the digest is
`1af388db958f569e4d6bd2229be43b09f2b15adae340031d30a8eaa99b934662`.

The payload filename mapping is part of the gist record contract. With one
fragment, snip removes a leading `NNN-` from its basename unless that would
produce an empty name or `README.md`; with multiple fragments it retains the
prefix. Included non-empty notes use `<fragment filename>.note.md`. An included
README uses `README.md`. Empty fragment or README content makes the payload
invalid; empty notes are omitted.

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

| Field | Type | Required | Validation |
|---|---|---|---|
| `schema_version` | integer | yes | `1` for this specification. |
| `entry_id` | string | yes | UUID simple form: exactly 32 hexadecimal characters. |
| `deleted_at` | RFC 3339 string | yes | Deletion time; snip emits UTC. |
| `original_path` | string | yes | Library-relative path beginning `snippets/`, using the path grammar above. |

`entry_id` identifies the trash entry, not the snippet — restore and purge
address entries by it. The directory name embeds the timestamp and title for
browsability and is, like other names, not authoritative.

Restoring without an override returns the package to `original_path`. Readers
MUST reject an absolute or escaping path, and restore MUST fail rather than
overwrite an occupied target.

## Runtime state under `.snip/`

`.snip/` holds coordination and crash-recovery state, not committed user data.
It MUST NOT be committed to version control. `snip init` excludes the entire
`.snip/` tree in `.gitignore`. `cache/` and stale lock files MAY be deleted while
no snip process is running. The entire tree MAY be deleted only when there is no
pending transaction; otherwise `backup/` can be the only surviving copy of a
package and MUST be retained until recovery or manual rescue.

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

Package replacements are staged, not edited in place, so an interrupted write
cannot leave a half-written snippet. A transaction directory has this shape;
`staged/` and `backup/` exist only during the phases that need them:

```text
.snip/transactions/4401c19cfc424373a6647224a2bc4553/
├── transaction.toml
├── staged/
└── backup/
```

```toml
schema_version = 1
operation = "replace"
original_path = "snippets/Scripts/Brewfile--79d92dea"
target_path = "snippets/Scripts/Brewfile--79d92dea"
```

`operation` MUST be `replace` in schema v1. Both paths MUST be safe relative
paths using `/`, with no empty, `.`, or `..` components; they MUST begin with
`snippets/` and resolve inside that directory. Recovery accepts legacy folder
components that are safe on the current platform even if they are not portable.
`target_path` differs from `original_path` when the change also renames or moves
the package, such as a retitle or a folder change.

Commit moves the original package to `backup/`, moves `staged/` to the target,
validates the committed target, then removes the transaction. Recovery after a
crash accepts an existing target only if it is a valid complete package;
otherwise it restores a valid backup. `snip doctor --repair` performs recovery
and reports what it did.

## Relationship to the CLI

The format is the contract; the CLI is one implementation of it. `snip doctor`
is a useful integrity check, not a complete conformance validator. In schema v1
it checks library and snippet semantic metadata, the tag registry, active
snippet packages, duplicate snippet UUIDs, canonical package-directory names,
folder portability, and pending package transactions; with `--repair` it also
attempts transaction recovery. Opening validates only enough root metadata to
identify a supported library. Ordinary scans enforce package structure, path,
UTF-8, and symbolic-link rules while leaving semantic metadata errors to
doctor. Tools that generate libraries SHOULD test the result with both `snip
doctor` and `snip list`.

The repository includes `tests/fixtures/conformance.sniplib` as a small golden
schema v1 library. It covers a missing `tags.toml`, unknown fields, a multibyte
title, an empty fragment, and a trash entry. The active snippet's fingerprint is
`633c2c21f9a0bf2e42fd65982ddf95486f91b764e746f6d9577ca8d6f1d3e089`;
the independent gist digest vector above covers the publication algorithm.
