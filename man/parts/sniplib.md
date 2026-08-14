%%DESCRIPTION
A snip library is a filesystem-native directory, conventionally ending in .sniplib. This manual specifies schema version 1 so independent tools can read and write libraries without depending on the snip executable. The terms MUST, MUST NOT, SHOULD, and MAY have their RFC 2119 meanings.

Every TOML manifest carries its own schema_version. Nested source, remotes, fragments, and similar records inherit the version of their containing manifest. Readers must reject schema version 0 or a version newer than they implement, symbolic links below snippets, managed paths that escape their package, and non-UTF-8 managed content.

Readers and writers must preserve unknown TOML values during read-modify-write so different tools do not discard one another's data. This does not preserve presentation: a writer may reorder fields, remove comments, or change whitespace when serializing a manifest. Writers must hold the library lock and leave a reader-valid state when interrupted.

%%VALIDATION SEVERITY
An unreadable or unparseable root snip.toml, a different format identifier, or an unsupported root schema prevents the library from opening.

An ambiguous or unsafe package cannot be loaded. This includes an escaping managed path, a missing managed file, any symbolic link below snippets, or non-UTF-8 managed content.

Semantic metadata errors do not make stored content unsafe. Readers should keep snippets with malformed timestamps or remote records available and report those errors through an integrity check such as snip doctor. Writers must not create new semantic errors.

%%LIBRARY ROOT
snip.toml identifies the library root. tags.toml is the tag registry. snippets contains the folder hierarchy and snippet packages. trash contains soft-deleted packages. .snip contains machine-local locks, transactions, and caches. .gitignore normally excludes .snip.

    Main.sniplib/
        snip.toml
        tags.toml
        snippets/
        trash/
        .snip/
        .gitignore

snip.toml, tags.toml, snippets, and trash are durable and should travel through version control. .snip is derived runtime state and must not be committed. User configuration is outside the library and is not part of this format.

Git does not preserve empty directories. Readers must recreate missing snippets, trash, and .snip directories on open; only snip.toml determines whether the root is a library.

%%LIBRARY MANIFEST
snip.toml contains format = "snip-library", schema_version = 1, a stable UUID id, a display name, and an RFC 3339 created_at timestamp.

    format = "snip-library"
    schema_version = 1
    id = "85f7c597-9c96-41f7-b2a0-b1cab232270b"
    name = "Main"
    created_at = "2026-07-22T17:00:00Z"

format must be snip-library. schema_version must be at least one. id is stable identity; name is presentation only. Writers emit created_at as RFC 3339, normally in UTC; readers may retain access while reporting a malformed legacy timestamp.

%%TAG REGISTRY
tags.toml starts with schema_version = 1 and contains [[tags]] entries. Each entry has a UUID id and name. color and source_id are optional; source_id records an imported identifier. A missing tags.toml is an empty registry, although omitting it loses metadata for unused tags.

The registry preserves identity and presentation even when no snippet currently uses a tag. A tag named by snippet.toml remains valid when absent from the registry, but writers should register newly used tags.

%%FOLDERS
Folders are ordinary directories below snippets. Their relative path is the folder path; there is no separate folder record. A package directly below snippets has the empty folder path, displayed as Uncategorized.

A directory containing snippet.toml is a package and readers must not descend into it looking for nested packages. An empty folder is represented by .keep; readers ignore that file when deciding whether a folder is empty.

%%SNIPPET PACKAGES
A package contains snippet.toml, an optional README.md, fragment files, optional per-fragment notes, and a reserved attachments directory.

    Brewfile--a5792745/
        snippet.toml
        README.md
        fragments/001-Brewfile
        notes/001.md
        attachments/

snippet.toml contains schema_version, UUID id, title, tags, pinned, locked, created_at, optional import source, optional remotes, and one or more [[fragments]]. The UUID is identity; titles need not be unique. Tags are trimmed, non-empty, and case-insensitively unique. locked asks writers to refuse mutation.

Each fragment has a UUID id unique within the snippet, title, language, file path, optional note path, and optional source_language. Fragment order is presentation order. file and note are package-relative, never absolute, never contain traversal, and must remain within the package after canonicalization. Managed files are UTF-8 and may be empty.

created_at is authoritative and writers emit it as RFC 3339. Optional source.modified_at follows the same timestamp format. Malformed legacy source or remote metadata is a semantic integrity error rather than a reason to hide otherwise safe content. modified_at is derived as the newest modification time among the manifest, README, fragments, and notes. Schema version 1 assigns no meaning to attachments and excludes them from fingerprints.

%%NAMING
Names are for human browsing and carry no identity. Writers sanitize a component by trimming, replacing control characters and slash, backslash, or colon with a collapsed dash, truncating to at most 80 UTF-8 bytes on a character boundary, then trimming spaces, dots, and dashes. An empty result becomes untitled.

Package directories use title-- plus the first eight hexadecimal UUID digits. Fragment files use fragments/NNN-title with an optional language-derived extension. Note files use notes/NNN.md. NNN is a 1-based position padded to three digits.

An extension is omitted when the sanitized title already contains a dot or is conventionally extensionless, including Brewfile, Dockerfile, Makefile, Justfile, and Procfile. Name drift after retitling is not corruption; snip organize re-derives cosmetic paths.

%%FINGERPRINT
The fingerprint is a BLAKE3 hash computed on every read and never stored. Its external representation is the 32-byte digest encoded as exactly 64 lowercase hexadecimal characters. It covers raw snippet.toml bytes, optional README.md, and each fragment file and optional note in manifest order. attachments and all other unreferenced files are excluded.

For every entry, hash the name length as little-endian u64, name bytes exactly as stored in the manifest, data length as little-endian u64, then data bytes. Length-prefixing makes renames distinct from content changes.

Fingerprints implement optimistic concurrency. A writer reads one value and asserts it at write time; if any managed byte or path changed, the write must fail instead of overwriting unseen work.

%%REMOTES
A snippet may contain one [[remotes]] entry per publication kind. Schema version 1 defines kind = "gist" with required non-empty host, id, and url. Other fields record public visibility, description, published filenames, whether notes and README were included, pushed_at, and pushed_digest. Duplicate kinds, malformed timestamps, and invalid remote fields are semantic integrity errors.

The files list contains unique non-empty names and identifies remote files owned by snip; later pushes must not delete unlisted files. pushed_digest hashes the publication payload rather than the snippet manifest, so recording the remote does not immediately make the digest stale. Its external representation is exactly 64 lowercase hexadecimal characters.

The digest feeds the length-prefixed domain tag snip-gist-payload-v1, then published filename/content pairs sorted by filename in ascending UTF-8 byte order, then the length-prefixed description. String lengths are UTF-8 byte lengths and integer lengths are little-endian u64 values.

Remote metadata carries no authority over local content. Removing an entry loses the link, not the snippet.

%%TRASH
Deletion moves an unchanged package under trash into a directory containing trash.toml and package. trash.toml contains schema_version, an unhyphenated UUID entry_id, RFC 3339 deleted_at, and the original library-relative path.

entry_id identifies the trash entry rather than the snippet. Restore returns the package to original_path unless another folder is requested. An occupied target is a conflict and must never be overwritten.

%%RUNTIME STATE
.snip is never user data and may be deleted while no snip process is running. It contains locks/library.lock, transactions, and a reserved cache. Writers hold the advisory exclusive library lock for mutations; readers do not require it. Cache entries must always be derivable from durable library data.

%%ATOMIC WRITES
A writer replacing one file must create a temporary file in the destination directory, flush its contents, and atomically rename it over the destination. Where supported, it should also sync the containing directory so the rename survives power loss.

A multi-file package change must stage and validate a complete replacement before swapping it into place. The transaction protocol below supplies schema version 1 recovery for that whole-package swap.

%%TRANSACTIONS
Package mutations are staged rather than edited in place. A transaction directory contains transaction.toml, a fully validated staged package, and during commit a backup of the prior package. transaction.toml records schema_version, operation, original_path, and target_path.

Commit swaps the backup out and the staged package in. Recovery prefers a complete committed target and otherwise restores the backup. snip doctor --repair performs this recovery.

%%CONFORMANCE
Direct filesystem edits are legal because the filesystem is authoritative, but a conforming writer must preserve unknown TOML values, acquire the lock, maintain manifest/file consistency, use atomic single-file replacement, and use an interruption-safe transaction for package changes. Run snip doctor after external or bulk writes.

Never write through symlinks, edit .snip as content, or write trash entries directly. Use the CLI for ordinary mutation because it supplies locking, validation, bookkeeping, and recovery.

%%SEE ALSO
snip(1), snip-init(1), snip-edit(1), snip-trash(1), snip-doctor(1), snip-agents(7)
