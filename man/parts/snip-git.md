%%DESCRIPTION
Run Git backup operations scoped to the resolved library. status reports repository availability, dirty files, upstream, and ahead or behind counts. init is idempotent. commit stages and commits library content. backup commits when dirty and pushes when ahead. push retries a push without committing, fetch updates remote-tracking refs, and pull fetches and merges the configured upstream.

CLI Git commands disable credential prompts and fail fast. They never switch branches. Use --ff-only with pull to reject divergence instead of creating a merge commit.

%%EXAMPLES
Initialize Git and commit the current library:

    snip git init
    snip git commit -m "Back up snippets"

Fetch remote status without changing worktree files, then make an idempotent backup:

    snip git fetch
    snip git backup

%%SEE ALSO
snip(1), snip-init(1), snip-doctor(1), snip-config(5), sniplib(5)
