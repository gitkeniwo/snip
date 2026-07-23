use snip::Library;
use snip::error::{Result, SnipError};
use snip::service::{purge_snippet, restore_snippet, trash_entries};

use super::output::{print_mutation, print_record, print_records};
use crate::cli::{OutputMode, PurgeArgs, RestoreArgs};

pub fn command_trash(library: &Library, output: OutputMode) -> Result<()> {
    let entries = trash_entries(library)?;
    if output == OutputMode::Human {
        for entry in entries {
            println!(
                "{}  {}  {}  {}",
                &entry.entry_id[..8],
                entry.deleted_at,
                entry.title,
                entry.original_path
            );
        }
    } else {
        print_records(&entries, output)?;
    }
    Ok(())
}

pub fn command_restore(library: &Library, args: &RestoreArgs, output: OutputMode) -> Result<()> {
    let snippet = restore_snippet(library, &args.selector, args.folder.as_deref())?;
    print_mutation(&snippet, None, output)
}

pub fn command_purge(library: &Library, args: &PurgeArgs, output: OutputMode) -> Result<()> {
    if !args.yes {
        return Err(SnipError::usage(
            "purge permanently deletes data; repeat with --yes",
        ));
    }
    let entry = purge_snippet(library, &args.selector)?;
    if output == OutputMode::Human {
        println!("permanently deleted: {} ({})", entry.title, entry.entry_id);
    } else {
        print_record(&entry, output)?;
    }
    Ok(())
}
