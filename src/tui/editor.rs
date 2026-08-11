use std::fs;
use std::path::Path;

use tempfile::Builder;
use uuid::Uuid;

use crate::domain::Fingerprint;
use crate::error::{ErrorKind, Result, SnipError};
use crate::external_editor::launch_editor;
use crate::filesystem::Library;
use crate::service::{EditOptions, edit_snippet};

#[derive(Clone, Debug)]
pub struct EditRequest {
    pub snippet_id: Uuid,
    pub target: EditTarget,
    pub expected: Fingerprint,
    pub original: String,
    pub edited: Option<String>,
    pub suffix: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditTarget {
    Content { fragment_id: Uuid },
    Note { fragment_id: Uuid },
    Readme,
}

#[derive(Debug)]
pub enum EditOutcome {
    Unchanged,
    Saved,
    Conflict(EditRequest),
}

pub fn run_external_edit(
    library: &Library,
    mut request: EditRequest,
    cwd: Option<&Path>,
    configured_editor: Option<&str>,
) -> Result<EditOutcome> {
    let suffix = if request.suffix.starts_with('.') {
        request.suffix.clone()
    } else {
        format!(".{}", request.suffix)
    };
    let mut temp = Builder::new()
        .suffix(&suffix)
        .tempfile()
        .map_err(|error| SnipError::io(format!("cannot create temporary editor file: {error}")))?;
    std::io::Write::write_all(&mut temp, request.original.as_bytes())?;
    launch_editor(temp.path(), cwd, configured_editor)?;
    let edited = fs::read_to_string(temp.path()).map_err(|error| {
        SnipError::io(format!(
            "cannot read editor result {}: {error}",
            temp.path().display()
        ))
    })?;
    if edited == request.original {
        return Ok(EditOutcome::Unchanged);
    }
    request.edited = Some(edited.clone());
    match save(library, &request, false) {
        Ok(()) => Ok(EditOutcome::Saved),
        Err(error) if error.kind == ErrorKind::Conflict => Ok(EditOutcome::Conflict(request)),
        Err(error) => Err(error),
    }
}

pub fn force_save(library: &Library, request: &EditRequest) -> Result<()> {
    save(library, request, true)
}

fn save(library: &Library, request: &EditRequest, force: bool) -> Result<()> {
    let edited = request
        .edited
        .as_ref()
        .ok_or_else(|| SnipError::usage("edited content is unavailable"))?;
    let mut options = EditOptions {
        if_hash: (!force).then(|| request.expected.clone()),
        force,
        ..EditOptions::default()
    };
    match &request.target {
        EditTarget::Content { fragment_id } => {
            options.fragment_selector = Some(fragment_id.to_string());
            options.content = Some(edited.clone());
        }
        EditTarget::Note { fragment_id } => {
            options.fragment_selector = Some(fragment_id.to_string());
            options.note = Some(Some(edited.clone()));
        }
        EditTarget::Readme => options.readme = Some(Some(edited.clone())),
    }
    edit_snippet(library, &request.snippet_id.to_string(), &options)?;
    Ok(())
}
