%%DESCRIPTION
Inspect and manage the manual pages embedded in the snip binary. path prints the installation directory, install writes pages and a hash manifest, uninstall removes only unchanged recorded pages, show opens an embedded page with the system viewer, and generate exports the embedded set.

Package-managed binaries should install manual pages through the same package manager. install refuses an existing unrecorded page unless --force is given. During upgrades it removes obsolete recorded pages whose hashes still match and keeps modified pages recorded for a later uninstall.

%%EXAMPLES
Install pages for the current user and show the create page:

    snip man install
    snip man show create

Export all embedded pages and install below a prefix:

    snip man generate ./generated-man
    snip man install --prefix /usr/local

%%SEE ALSO
snip(1), snip-config(5), snip-agents(7)
