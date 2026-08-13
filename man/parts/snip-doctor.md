%%DESCRIPTION
Validate library manifests, paths, content, and interrupted transaction state. The report includes checked items, errors, warnings, pending transactions, repairs, and an overall ok flag.

With --repair, snip finishes or rolls back interrupted staged writes. It does not invent missing snippet content or bypass ordinary validation failures.

%%EXAMPLES
Check the resolved library and emit a machine-readable report:

    snip --output json doctor

Repair interrupted transactions in a specific library:

    snip --library ~/Notes.sniplib doctor --repair

%%SEE ALSO
snip(1), snip-init(1), snip-edit(1), sniplib(5)
