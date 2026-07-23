use serde_json::json;
use snip::Library;
use snip::error::{Result, SnipError};
use snip::service::{create_folder, delete_folder, delete_tag, move_folder, rename_tag};
use std::path::Path;

use super::output::{print_count, print_record, print_records, print_simple_path};
use crate::cli::{FolderArgs, FolderCommand, OutputMode, TagArgs, TagCommand};

pub fn command_folder(library: &Library, args: &FolderArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        FolderCommand::List => {
            let folders = library.scan()?.folders;
            if output == OutputMode::Human {
                for folder in folders {
                    println!("{folder}");
                }
            } else {
                print_records(&folders, output)?;
            }
        }
        FolderCommand::Create { folder } => {
            let path = create_folder(library, folder)?;
            print_simple_path("created", &path, output)?;
        }
        FolderCommand::Rename { folder, new_name } => {
            if Path::new(new_name).components().count() != 1 {
                return Err(SnipError::usage(
                    "new folder name must be one path component",
                ));
            }
            let source = Path::new(folder);
            let target = source
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(new_name);
            let path = move_folder(library, folder, &target.to_string_lossy())?;
            print_simple_path("renamed", &path, output)?;
        }
        FolderCommand::Move { folder, target } => {
            let path = move_folder(library, folder, target)?;
            print_simple_path("moved", &path, output)?;
        }
        FolderCommand::Delete { folder } => {
            delete_folder(library, folder)?;
            if output == OutputMode::Human {
                println!("deleted folder: {folder}");
            } else {
                print_record(&json!({"deleted": folder}), output)?;
            }
        }
    }
    Ok(())
}

pub fn command_tag(library: &Library, args: &TagArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        TagCommand::List => {
            let tags = library.scan()?.tags;
            if output == OutputMode::Human {
                for tag in tags {
                    println!("{tag}");
                }
            } else {
                print_records(&tags, output)?;
            }
        }
        TagCommand::Rename { tag, new_name } => {
            let changed = rename_tag(library, tag, new_name)?;
            print_count("updated_snippets", changed, output)?;
        }
        TagCommand::Delete { tag } => {
            let changed = delete_tag(library, tag)?;
            print_count("updated_snippets", changed, output)?;
        }
    }
    Ok(())
}
