use std::collections::BTreeMap;
use std::path::Path;

use crate::domain::{FragmentManifest, Snippet};
use crate::error::{Result, SnipError};

#[derive(Debug)]
pub struct PayloadOptions {
    pub include_notes: bool,
    pub include_readme: bool,
}

#[derive(Debug)]
pub struct Payload {
    pub description: String,
    pub files: BTreeMap<String, String>,
}

pub fn build(snippet: &Snippet, description: &str, options: &PayloadOptions) -> Result<Payload> {
    let single = snippet.loaded_fragments.len() == 1;
    let mut files = BTreeMap::new();
    let mut empty = Vec::new();
    for fragment in &snippet.loaded_fragments {
        let base = gist_filename(&fragment.manifest, single);
        collect(&mut files, &mut empty, &base, &fragment.content);
        if options.include_notes
            && let Some(filename) = fragment_note_filename(&base, &fragment.manifest)
            && let Some(content) = &fragment.note_content
            && !content.trim().is_empty()
        {
            files.insert(filename, content.clone());
        }
    }
    if options.include_readme
        && let Some(content) = &snippet.readme
    {
        collect(&mut files, &mut empty, "README.md", content);
    }
    if !empty.is_empty() {
        return Err(SnipError::validation(format!(
            "cannot publish empty files to a gist: {}",
            empty.join(", ")
        ))
        .with_hint("GitHub rejects gist files with no content"));
    }
    if files.is_empty() {
        return Err(SnipError::validation(format!(
            "snippet {} has nothing to publish",
            snippet.title
        )));
    }
    Ok(Payload {
        description: description.to_owned(),
        files,
    })
}

fn collect(
    files: &mut BTreeMap<String, String>,
    empty: &mut Vec<String>,
    filename: &str,
    content: &str,
) {
    if content.trim().is_empty() {
        empty.push(filename.to_owned());
    } else {
        files.insert(filename.to_owned(), content.to_owned());
    }
}

/// Gist filename for a fragment's content: the basename of its manifest `file`
/// field, with the `NNN-` index prefix stripped when the snippet has exactly
/// one fragment.
fn gist_filename(manifest: &FragmentManifest, single: bool) -> String {
    let basename = Path::new(&manifest.file)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&manifest.file);
    if single
        && let Some(rest) = strip_index_prefix(basename)
        && !rest.is_empty()
        && !rest.eq_ignore_ascii_case("README.md")
    {
        return rest.to_owned();
    }
    basename.to_owned()
}

/// The gist filename for a fragment's note, or `None` when notes are not
/// included or the fragment has no note content.
fn fragment_note_filename(base: &str, manifest: &FragmentManifest) -> Option<String> {
    if manifest.note.is_some() {
        Some(format!("{base}.note.md"))
    } else {
        None
    }
}

fn strip_index_prefix(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() >= 4 && bytes[..3].iter().all(u8::is_ascii_digit) && bytes[3] == b'-' {
        Some(&value[4..])
    } else {
        None
    }
}

pub fn digest(payload: &Payload) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&20u64.to_le_bytes());
    hasher.update(b"snip-gist-payload-v1");
    for (filename, content) in &payload.files {
        hasher.update(&(filename.len() as u64).to_le_bytes());
        hasher.update(filename.as_bytes());
        hasher.update(&(content.len() as u64).to_le_bytes());
        hasher.update(content.as_bytes());
    }
    hasher.update(&(payload.description.len() as u64).to_le_bytes());
    hasher.update(payload.description.as_bytes());
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Fingerprint, Fragment, SnippetManifest};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn manifest_fragment(index: usize, name: &str, note: bool) -> FragmentManifest {
        FragmentManifest {
            id: Uuid::new_v4(),
            title: name.to_owned(),
            language: "text".to_owned(),
            file: format!("fragments/{index:03}-{name}"),
            note: note.then_some(format!("notes/{index:03}.md")),
            source_language: None,
            extra: toml::Table::new(),
        }
    }

    fn snippet(fragments: Vec<(FragmentManifest, String, Option<String>)>) -> Snippet {
        let loaded_fragments = fragments
            .into_iter()
            .map(|(manifest, content, note_content)| Fragment {
                manifest,
                content,
                note_content,
                absolute_path: PathBuf::new(),
            })
            .collect::<Vec<_>>();
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Brewfile".to_owned(),
                tags: Vec::new(),
                pinned: false,
                locked: false,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                source: None,
                remotes: Vec::new(),
                fragments: loaded_fragments
                    .iter()
                    .map(|fragment| fragment.manifest.clone())
                    .collect(),
                extra: toml::Table::new(),
            },
            readme: None,
            folder: String::new(),
            package_path: PathBuf::new(),
            modified_at: None,
            fingerprint: Fingerprint("test".to_owned()),
            loaded_fragments,
        }
    }

    fn one_fragment(note: bool) -> Snippet {
        snippet(vec![(
            manifest_fragment(1, "Brewfile", note),
            "tap 'homebrew/bundle'\n".to_owned(),
            note.then_some("Installs the bundle.\n".to_owned()),
        )])
    }

    fn two_fragments() -> Snippet {
        snippet(vec![
            (
                manifest_fragment(1, "setup.sh", false),
                "brew install\n".to_owned(),
                None,
            ),
            (
                manifest_fragment(2, "teardown.sh", false),
                "brew cleanup\n".to_owned(),
                None,
            ),
        ])
    }

    #[test]
    fn single_fragment_strips_the_index_prefix() {
        let payload = build(&one_fragment(false), "Brewfile", &default_options()).unwrap();
        assert_eq!(
            payload.files.keys().cloned().collect::<Vec<_>>(),
            ["Brewfile"]
        );
    }

    #[test]
    fn two_fragments_keep_both_prefixes_for_alphabetical_order() {
        let payload = build(&two_fragments(), "Brewfile", &default_options()).unwrap();
        assert_eq!(
            payload.files.keys().cloned().collect::<Vec<_>>(),
            ["001-setup.sh", "002-teardown.sh"]
        );
    }

    #[test]
    fn a_readme_titled_fragment_keeps_its_prefix_to_avoid_collision() {
        let mut snippet = one_fragment(false);
        snippet.loaded_fragments[0].manifest.file = "fragments/001-README.md".to_owned();
        snippet.readme = Some("# Brewfile\n".to_owned());
        let payload = build(&snippet, "Brewfile", &default_options()).unwrap();
        let keys = payload.files.keys().cloned().collect::<Vec<_>>();
        assert!(keys.contains(&"001-README.md".to_owned()));
        assert!(keys.contains(&"README.md".to_owned()));
    }

    #[test]
    fn a_prefix_only_fragment_keeps_the_prefix() {
        let mut snippet = one_fragment(false);
        snippet.loaded_fragments[0].manifest.file = "fragments/001-".to_owned();
        let payload = build(&snippet, "Brewfile", &default_options()).unwrap();
        assert_eq!(payload.files.keys().cloned().collect::<Vec<_>>(), ["001-"]);
    }

    #[test]
    fn include_notes_appends_a_note_file_and_skips_fragments_without_notes() {
        let options = PayloadOptions {
            include_notes: true,
            include_readme: true,
        };
        let payload = build(&one_fragment(true), "Brewfile", &options).unwrap();
        assert_eq!(
            payload.files.keys().cloned().collect::<Vec<_>>(),
            ["Brewfile", "Brewfile.note.md"]
        );

        let payload = build(&two_fragments(), "Brewfile", &options).unwrap();
        assert_eq!(
            payload.files.keys().cloned().collect::<Vec<_>>(),
            ["001-setup.sh", "002-teardown.sh"]
        );
    }

    #[test]
    fn empty_notes_are_skipped_while_their_fragment_is_published() {
        let options = PayloadOptions {
            include_notes: true,
            include_readme: true,
        };
        let mut snippet = one_fragment(true);
        snippet.loaded_fragments[0].note_content = Some("  \n".to_owned());
        let payload = build(&snippet, "Brewfile", &options).unwrap();
        assert_eq!(
            payload.files.keys().cloned().collect::<Vec<_>>(),
            ["Brewfile"]
        );
        assert_eq!(payload.files["Brewfile"], "tap 'homebrew/bundle'\n");
    }

    #[test]
    fn include_readme_false_omits_the_readme() {
        let mut snippet = one_fragment(false);
        snippet.readme = Some("# Brewfile\n".to_owned());
        let options = PayloadOptions {
            include_notes: false,
            include_readme: false,
        };
        let payload = build(&snippet, "Brewfile", &options).unwrap();
        assert!(!payload.files.contains_key("README.md"));
    }

    #[test]
    fn empty_fragments_are_rejected_naming_every_offender() {
        let mut snippet = two_fragments();
        snippet.loaded_fragments[0].content = "   \n".to_owned();
        snippet.loaded_fragments[1].content = String::new();
        let error = build(&snippet, "Brewfile", &default_options())
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "cannot publish empty files to a gist: 001-setup.sh, 002-teardown.sh"
        );
    }

    #[test]
    fn digest_is_stable_for_a_fixed_payload() {
        let payload = Payload {
            description: "Brewfile".to_owned(),
            files: BTreeMap::from([
                (
                    "001-Brewfile".to_owned(),
                    "tap 'homebrew/bundle'\n".to_owned(),
                ),
                ("README.md".to_owned(), "# Brewfile\n".to_owned()),
            ]),
        };
        assert_eq!(
            digest(&payload),
            "1af388db958f569e4d6bd2229be43b09f2b15adae340031d30a8eaa99b934662"
        );
    }

    #[test]
    fn digest_changes_when_the_description_changes_but_files_do_not() {
        let files = BTreeMap::from([("a.py".to_owned(), "print('hi')\n".to_owned())]);
        let first = Payload {
            description: "one".to_owned(),
            files: files.clone(),
        };
        let second = Payload {
            description: "two".to_owned(),
            files,
        };
        assert_ne!(digest(&first), digest(&second));
    }

    #[test]
    fn digest_is_unchanged_by_insertion_order() {
        let mut one = BTreeMap::new();
        one.insert("a.py".to_owned(), "aaa\n".to_owned());
        one.insert("b.py".to_owned(), "bbb\n".to_owned());
        let mut two = BTreeMap::new();
        two.insert("b.py".to_owned(), "bbb\n".to_owned());
        two.insert("a.py".to_owned(), "aaa\n".to_owned());
        assert_eq!(
            digest(&Payload {
                description: "x".to_owned(),
                files: one,
            }),
            digest(&Payload {
                description: "x".to_owned(),
                files: two,
            })
        );
    }

    fn default_options() -> PayloadOptions {
        PayloadOptions {
            include_notes: false,
            include_readme: true,
        }
    }
}
