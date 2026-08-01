pub mod gh;
pub mod payload;

use std::collections::BTreeMap;

use serde::Serialize;

use crate::domain::{Fingerprint, RemoteRecord, Snippet};
use crate::error::{Result, SnipError};
use crate::filesystem::{Library, now_rfc3339};
use crate::service::snippet::{ensure_hash, replace_package};

pub use gh::{Gist, GistFile, Unavailable};
pub use payload::{Payload, PayloadOptions};

pub struct PushOptions {
    pub public: bool,
    pub description: Option<String>,
    pub new: bool,
    pub include_notes: bool,
    pub include_readme: bool,
    pub if_hash: Option<String>,
    pub force: bool,
}

pub enum PushOutcome {
    Created {
        snippet: Snippet,
        record: RemoteRecord,
    },
    Updated {
        snippet: Snippet,
        record: RemoteRecord,
    },
    Unchanged {
        snippet: Snippet,
        record: RemoteRecord,
    },
}

impl PushOutcome {
    pub fn action(&self) -> &'static str {
        match self {
            Self::Created { .. } => "created",
            Self::Updated { .. } => "updated",
            Self::Unchanged { .. } => "unchanged",
        }
    }

    pub fn snippet(&self) -> &Snippet {
        match self {
            Self::Created { snippet, .. }
            | Self::Updated { snippet, .. }
            | Self::Unchanged { snippet, .. } => snippet,
        }
    }

    pub fn record(&self) -> &RemoteRecord {
        match self {
            Self::Created { record, .. }
            | Self::Updated { record, .. }
            | Self::Unchanged { record, .. } => record,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusState {
    Unlinked,
    Clean,
    Modified,
    Missing,
}

impl StatusState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Unlinked => "unlinked",
            Self::Clean => "clean",
            Self::Modified => "modified",
            Self::Missing => "missing",
        }
    }
}

#[derive(Serialize)]
pub struct StatusReport {
    pub snippet: Snippet,
    pub state: StatusState,
    #[serde(rename = "gist")]
    pub record: Option<RemoteRecord>,
}

pub fn find(snippet: &Snippet) -> Option<&RemoteRecord> {
    snippet
        .manifest
        .remotes
        .iter()
        .find(|record| record.kind == "gist")
}

pub fn push(library: &Library, selector: &str, options: &PushOptions) -> Result<PushOutcome> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    if let Some(hash) = &options.if_hash {
        ensure_hash(&snippet, Some(&Fingerprint(hash.clone())))?;
    }
    let existing = if options.new {
        None
    } else {
        find(&snippet).cloned()
    };
    let description = options
        .description
        .clone()
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|record| record.description.clone())
        })
        .unwrap_or_else(|| snippet.title.clone());
    let payload = payload::build(
        &snippet,
        &description,
        &PayloadOptions {
            include_notes: options.include_notes,
            include_readme: options.include_readme,
        },
    )?;
    let digest = payload::digest(&payload);

    if let Some(record) = &existing
        && record.pushed_digest.as_deref() == Some(digest.as_str())
        && !options.force
    {
        return Ok(PushOutcome::Unchanged {
            snippet,
            record: record.clone(),
        });
    }

    let gist = if let Some(record) = &existing {
        if options.public && !record.public {
            return Err(SnipError::validation(
                "gist visibility cannot be changed after creation",
            ));
        }
        let mut files = BTreeMap::new();
        for (name, content) in &payload.files {
            files.insert(name.clone(), Some(content.clone()));
        }
        for recorded in &record.files {
            if !payload.files.contains_key(recorded) {
                files.insert(recorded.clone(), None);
            }
        }
        gh::update(&record.id, &description, &files)?
    } else {
        gh::create(&description, options.public, &payload.files)?
    };

    let record = record_from_gist(
        &gist,
        &description,
        &payload.files.keys().cloned().collect::<Vec<_>>(),
        Some(now_rfc3339()?),
        Some(digest),
    );
    let result = write_record(library, &snippet, &record)?;

    if existing.is_some() {
        Ok(PushOutcome::Updated {
            snippet: result,
            record,
        })
    } else {
        Ok(PushOutcome::Created {
            snippet: result,
            record,
        })
    }
}

pub fn attach(library: &Library, selector: &str, gist: &str) -> Result<Snippet> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    if let Some(existing) = find(&snippet) {
        return Err(SnipError::conflict(format!(
            "snippet {} is already linked to gist {}",
            snippet.title, existing.id
        ))
        .with_hint("run snip gist detach <selector> first"));
    }
    let id = parse_gist_id(gist)?;
    let fetched = gh::fetch(&id)?;
    let record = record_from_gist(
        &fetched,
        fetched.description.as_deref().unwrap_or(&snippet.title),
        &fetched.files.keys().cloned().collect::<Vec<_>>(),
        None,
        None,
    );
    write_record(library, &snippet, &record)
}

pub fn detach(library: &Library, selector: &str) -> Result<(Snippet, RemoteRecord)> {
    let (snippet, record) = prepare_linked(library, selector)?;
    let result = remove_record(library, &snippet)?;
    Ok((result, record))
}

pub fn delete(library: &Library, selector: &str) -> Result<(Snippet, RemoteRecord)> {
    let (snippet, record) = prepare_linked(library, selector)?;
    gh::delete(&record.id)?;
    let result = remove_record(library, &snippet)?;
    Ok((result, record))
}

pub fn status(snippet: &Snippet) -> Result<StatusReport> {
    let Some(record) = find(snippet) else {
        return Ok(StatusReport {
            snippet: snippet.clone(),
            state: StatusState::Unlinked,
            record: None,
        });
    };
    let description = record
        .description
        .clone()
        .unwrap_or_else(|| snippet.title.clone());
    let payload = payload::build(
        snippet,
        &description,
        &PayloadOptions {
            include_notes: false,
            include_readme: true,
        },
    )?;
    let digest = payload::digest(&payload);
    let state = if record.pushed_digest.as_deref() == Some(digest.as_str()) {
        StatusState::Clean
    } else {
        StatusState::Modified
    };
    Ok(StatusReport {
        snippet: snippet.clone(),
        state,
        record: Some(record.clone()),
    })
}

pub fn parse_gist_id(input: &str) -> Result<String> {
    let input = input.trim();
    if input.is_empty() {
        return Err(SnipError::usage(format!("not a gist ID or URL: {input}")));
    }
    let candidate = if is_hex_run(input) {
        Some(input)
    } else {
        input
            .split(['/', '#', '?'])
            .filter(|segment| !segment.is_empty())
            .rfind(|segment| is_hex_run(segment))
    };
    candidate
        .map(str::to_owned)
        .ok_or_else(|| SnipError::usage(format!("not a gist ID or URL: {input}")))
}

fn is_hex_run(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn record_from_gist(
    gist: &Gist,
    description: &str,
    files: &[String],
    pushed_at: Option<String>,
    pushed_digest: Option<String>,
) -> RemoteRecord {
    RemoteRecord {
        kind: "gist".to_owned(),
        host: gh::derive_host(&gist.html_url),
        id: gist.id.clone(),
        url: gist.html_url.clone(),
        public: gist.public,
        description: Some(description.to_owned()),
        files: files.to_vec(),
        pushed_at,
        pushed_digest,
        extra: toml::Table::new(),
    }
}

fn write_record(library: &Library, snippet: &Snippet, record: &RemoteRecord) -> Result<Snippet> {
    let record = record.clone();
    replace_package(
        library,
        snippet,
        &snippet.package_path,
        |_stage, manifest| {
            if let Some(index) = manifest
                .remotes
                .iter()
                .position(|entry| entry.kind == "gist")
            {
                manifest.remotes[index] = record.clone();
            } else {
                manifest.remotes.push(record.clone());
            }
            Ok(())
        },
    )
}

fn remove_record(library: &Library, snippet: &Snippet) -> Result<Snippet> {
    replace_package(
        library,
        snippet,
        &snippet.package_path,
        |_stage, manifest| {
            manifest.remotes.retain(|entry| entry.kind != "gist");
            Ok(())
        },
    )
}

fn prepare_linked(library: &Library, selector: &str) -> Result<(Snippet, RemoteRecord)> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    let record = find(&snippet).cloned().ok_or_else(|| {
        SnipError::not_found(format!("snippet {} has no gist", snippet.title))
            .with_hint("run: snip gist push <selector>")
    })?;
    Ok((snippet, record))
}

#[cfg(test)]
mod tests {
    use super::{RemoteRecord, parse_gist_id};
    use toml::Value;

    #[test]
    fn parse_gist_id_accepts_bare_ids_urls_and_trailing_slashes() {
        let id = "5b0e0062eb8e9654adad7bb1d81cc75f";
        assert_eq!(parse_gist_id(id).unwrap(), id);
        assert_eq!(
            parse_gist_id(&format!("https://gist.github.com/octocat/{id}")).unwrap(),
            id
        );
        assert_eq!(
            parse_gist_id(&format!("https://gist.github.com/octocat/{id}/")).unwrap(),
            id
        );
    }

    #[test]
    fn parse_gist_id_rejects_empty_and_non_hex_words() {
        assert_eq!(
            parse_gist_id("").unwrap_err().to_string(),
            "not a gist ID or URL: "
        );
        assert_eq!(
            parse_gist_id("not-a-gist").unwrap_err().to_string(),
            "not a gist ID or URL: not-a-gist"
        );
        assert_eq!(
            parse_gist_id("https://gist.github.com/user/nope")
                .unwrap_err()
                .to_string(),
            "not a gist ID or URL: https://gist.github.com/user/nope"
        );
    }

    #[test]
    fn remote_record_round_trips_preserving_unknown_keys() {
        let mut extra = toml::Table::new();
        extra.insert("future_field".to_owned(), Value::String("kept".to_owned()));
        let record = RemoteRecord {
            kind: "gist".to_owned(),
            host: "github.com".to_owned(),
            id: "5b0e0062eb8e9654adad7bb1d81cc75f".to_owned(),
            url: "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f".to_owned(),
            public: false,
            description: Some("Brewfile".to_owned()),
            files: vec!["001-Brewfile".to_owned(), "README.md".to_owned()],
            pushed_at: Some("2026-08-01T10:00:00Z".to_owned()),
            pushed_digest: Some("3f8a…".to_owned()),
            extra,
        };
        let text = toml::to_string(&record).unwrap();
        let decoded: RemoteRecord = toml::from_str(&text).unwrap();
        assert_eq!(decoded.kind, "gist");
        assert_eq!(decoded.description.as_deref(), Some("Brewfile"));
        assert_eq!(decoded.files, vec!["001-Brewfile", "README.md"]);
        assert_eq!(
            decoded.extra.get("future_field").and_then(Value::as_str),
            Some("kept")
        );
    }
}
