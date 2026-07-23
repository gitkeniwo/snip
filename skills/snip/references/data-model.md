# snip on-disk data model

Read this when the user asks what a file in their library is, wants bulk or
scripted changes, is migrating data in or out, or when something looks corrupt.
`FORMAT.md` in the snip repository is the normative specification; this is the
working summary plus the rules for touching files directly.

## Layout

```
Main.sniplib/
├── snip.toml            # library manifest: format, schema_version, id, name, created_at
├── tags.toml            # tag registry, including tags no snippet currently uses
├── snippets/            # folder hierarchy == real directories
│   └── Scripts/
│       └── Greeter--79d92dea/     # a snippet package
│           ├── snippet.toml       # manifest: identity + metadata + fragment list
│           ├── README.md          # optional, snippet-level
│           ├── fragments/001-Greeter.sh
│           ├── notes/001.md       # optional, per-fragment
│           └── attachments/
├── trash/               # soft-deleted packages, each with trash.toml + package/
└── .snip/               # runtime only: locks, transactions, caches — NOT user data
```

Nothing under `.snip/` is user data. It is safe to delete when snip is not
running, and it must never be committed to version control or copied as if it
were content.

## Identity

`snippet.toml` carries the identity:

```toml
schema_version = 1
id = "a5792745-36aa-36ea-9966-f301ff14f3f0"
title = "Brewfile"
tags = ["dotfiles", "homebrew"]
pinned = false
locked = false
created_at = "2026-03-15T10:20:00Z"

[[fragments]]
id = "f22e0f61-ef44-4021-9380-5ec4842b80b5"
title = "Fragment"
language = "makefile"
file = "fragments/001-Brewfile"
note = "notes/001.md"
```

The **UUID is the identity**; the directory name (`Greeter--79d92dea`) is a
human-readable convenience and the `--79d92dea` suffix is just a UUID prefix.
Renaming a package directory does not change which snippet it is — `snip
organize` exists precisely to re-derive tidy names from titles.

A snippet's folder is its parent path under `snippets/`. There is no folder
record anywhere; folders exist because directories exist. A folder with no
snippets is an ordinary empty directory.

Rules the format requires: at least one fragment; fragment and note paths
relative and inside the package, never through a symlink; content valid UTF-8
(may be empty); tags trimmed, non-empty, and unique case-insensitively.

**Unknown TOML fields must survive a read-modify-write.** snip preserves fields
it does not recognize so newer versions and other tools can coexist. If you
rewrite a manifest yourself, preserve them too.

## Fingerprints

The fingerprint is BLAKE3 over the manifest bytes, the README, and each
fragment/note path and its bytes in manifest order. It is **computed on every
read, never stored**, which has two consequences worth remembering:

- Any change to any managed file changes the fingerprint. There is no way to
  edit content without invalidating a fingerprint you are holding.
- A fingerprint you read is only a claim about a moment in time. That is exactly
  why `--if-hash` exists: it re-computes at write time and refuses if the value
  moved.

`modified_at` is not stored either — it is the newest mtime among managed files.
`created_at` in the manifest is authoritative.

## Writing files directly

The filesystem is the source of truth, so direct edits are legal and snip will
pick them up on its next scan. But the CLI provides things a plain write does
not:

- an **advisory lock**, so a concurrent TUI or CLI write cannot interleave
- a **staged transaction** — snip builds the new package and swaps it in, so a
  crash leaves either the old or the new state, never a half-written one
- **manifest bookkeeping**: fragment ordering, file naming and extensions, tag
  registration in `tags.toml`, and fingerprint recomputation

So prefer the CLI. Direct writes are the right call in a narrow set of cases:

- **Bulk migration or import** of many snippets at once, where per-snippet CLI
  calls would be unreasonably slow. Run `snip doctor` afterward.
- **Reading**, always fine — but `snip cat` and `snip path` already give you
  content and paths without hand-parsing TOML.
- **Repairing** a library the CLI refuses to open, guided by `snip doctor`.

If you do write directly: never do it while a TUI is open on that library, keep
the manifest and the files consistent with each other, preserve unknown fields,
and finish with `snip doctor` to confirm the result validates.

Never hand-edit anything under `.snip/`, and never write into `trash/` — use
`snip restore` and `snip purge`, which maintain `trash.toml`.

## Recovering

`snip doctor` reports `checked`, `errors`, `warnings`, `pending_transactions`,
`repaired`, and `ok`. `snip doctor --repair` finishes or rolls back interrupted
transactions left by a killed process.

A library under Git is a good safety net for bulk work — `snip git status`,
`snip git diff`, and `snip git commit` operate scoped to the library directory,
so you can inspect exactly what a bulk change did before committing it.
