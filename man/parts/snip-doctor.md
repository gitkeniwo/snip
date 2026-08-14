%%DESCRIPTION
Validate library manifests, semantic metadata, paths, content, and interrupted transaction state. The report includes checked items, errors, warnings, pending transactions, repairs, and an overall ok flag.

doctor checks the library and snippet created_at timestamps, optional source timestamps, remote records and pushed digests, the tag registry, package structure, duplicate snippet UUIDs, canonical package names, folder portability, and pending transactions. Ordinary scans keep structurally safe content readable when only semantic timestamps or remote metadata are malformed; doctor reports those integrity errors and sets ok to false.

With --repair, snip finishes or rolls back interrupted staged writes. It does not invent missing snippet content or bypass ordinary validation failures.

%%EXAMPLES
Check the resolved library and emit a machine-readable report:

    snip --output json doctor

Repair interrupted transactions in a specific library:

    snip --library ~/Notes.sniplib doctor --repair

%%SEE ALSO
snip(1), snip-init(1), snip-edit(1), sniplib(5)
