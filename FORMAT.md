# snip library format v1

The format is a directory protocol. UTF-8 text files are authoritative; runtime
locks, transactions, and future indexes under `.snip/` are not user data.

## Library manifest

`snip.toml` identifies the root:

```toml
format = "snip-library"
schema_version = 1
id = "85f7c597-9c96-41f7-b2a0-b1cab232270b"
name = "Main"
created_at = "2026-07-22T17:00:00Z"
```

Readers must reject a schema version newer than they support before writing.
Unknown TOML fields must survive read-modify-write operations.

## Tag registry

`tags.toml` preserves stable tag definitions, including tags not currently used
by a snippet:

```toml
schema_version = 1

[[tags]]
id = "31296bf7-e575-46c4-a906-f4e22a4019e9"
name = "ffmpeg"
color = 0
source_id = "31296BF7-E575-46C4-A906-F4E22A4019E9"
```

A tag found directly in `snippet.toml` remains valid even if it is absent from
the registry. CLI writes add new tags to the registry.

## Snippet package

Any directory below `snippets/` containing `snippet.toml` is a snippet package.
The parent path is its logical folder; directory names are not identifiers.

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

At least one fragment is required. Fragment and note paths must be relative,
must stay inside the package, and must not resolve through symbolic links.
Content may be empty but must be valid UTF-8. `README.md` is an optional
snippet-level description; `notes/` holds per-fragment Markdown.

Tags are trimmed, non-empty, and unique under case-insensitive comparison.
`created_at` is authoritative; the effective modified time is the newest mtime
among managed package files.

The fingerprint is BLAKE3 over the raw manifest, README, and manifest-ordered
fragment/note path and bytes. It is computed rather than stored.

## Trash and runtime state

Each trash entry contains `trash.toml` plus the package under `package/`.
`trash.toml` records the entry UUID, deletion time, and original library-relative
path.

Transactions under `.snip/transactions/` contain a validated staged package,
the previous package backup during commit, and `transaction.toml`. Recovery
prefers a complete committed target; otherwise it restores the backup.
