%%DESCRIPTION
Manage soft-deleted snippets. delete moves a snippet package into the library trash and records its original path. trash lists entries and their entry_id. restore moves an entry back, optionally to another folder. purge permanently removes an entry and requires --yes.

restore and purge select a trash entry by entry_id, not by the snippet UUID. A restore fails with conflict when the destination is occupied. Purge is irreversible.

%%EXAMPLES
Soft-delete a snippet after checking its fingerprint:

    snip delete 79d92dea --if-hash 472697ff761e33cf

List trash as JSON and restore one entry elsewhere:

    snip --output json trash
    snip restore 4401c19c --folder Recovered

%%SEE ALSO
snip(1), snip-query(1), snip-edit(1), sniplib(5)
