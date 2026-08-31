use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::{CatalogSnapshot, Fingerprint, Fragment, Snippet, SnippetManifest};
use crate::error::{Result, SnipError};

use super::io::{
    hash_entry, read_safe_file, reject_symlink, reject_symlinks_recursive,
    validate_snippet_manifest,
};
use super::library::Library;
use super::paths::{normalize_tags, path_to_slashes, resolve_managed_path, system_time_rfc3339};

pub(crate) const SNIPPET_MANIFEST: &str = "snippet.toml";

impl Library {
    pub fn scan(&self) -> Result<CatalogSnapshot> {
        let mut package_paths = Vec::new();
        let mut folders = BTreeSet::new();
        walk_snippets(
            &self.snippets_dir(),
            &self.snippets_dir(),
            &mut package_paths,
            &mut folders,
        )?;
        package_paths.sort();
        let mut snippets = package_paths
            .iter()
            .map(|path| self.load_snippet(path))
            .collect::<Result<Vec<_>>>()?;
        snippets.sort_by(|left, right| {
            left.folder
                .to_lowercase()
                .cmp(&right.folder.to_lowercase())
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut ids = HashSet::new();
        for snippet in &snippets {
            if !ids.insert(snippet.id) {
                return Err(SnipError::validation(format!(
                    "duplicate snippet UUID: {}",
                    snippet.id
                )));
            }
        }
        let mut tag_map = BTreeMap::<String, String>::new();
        for tag in self.tag_registry()?.tags {
            tag_map.insert(tag.name.to_lowercase(), tag.name);
        }
        for snippet in &snippets {
            for tag in &snippet.tags {
                tag_map
                    .entry(tag.to_lowercase())
                    .or_insert_with(|| tag.clone());
            }
        }
        Ok(CatalogSnapshot {
            library: self.manifest.clone(),
            root: self.root.clone(),
            snippets,
            folders: folders.into_iter().collect(),
            tags: tag_map.into_values().collect(),
        })
    }

    pub fn load_snippet(&self, package_path: &Path) -> Result<Snippet> {
        reject_symlinks_recursive(package_path)?;
        let manifest_path = package_path.join(SNIPPET_MANIFEST);
        reject_symlink(&manifest_path)?;
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
            SnipError::validation(format!("cannot read {}: {error}", manifest_path.display()))
        })?;
        let manifest_text = std::str::from_utf8(&manifest_bytes).map_err(|error| {
            SnipError::validation(format!("{} is not UTF-8: {error}", manifest_path.display()))
        })?;
        let mut manifest: SnippetManifest = toml::from_str(manifest_text).map_err(|error| {
            SnipError::validation(format!("cannot parse {}: {error}", manifest_path.display()))
        })?;
        validate_snippet_manifest(&manifest, &manifest_path)?;
        manifest.tags = normalize_tags(&manifest.tags)?;

        let mut hasher = blake3::Hasher::new();
        hash_entry(&mut hasher, SNIPPET_MANIFEST, &manifest_bytes);
        let mut modified = fs::metadata(&manifest_path)
            .and_then(|value| value.modified())
            .ok();

        let readme_path = package_path.join("README.md");
        let readme = if readme_path.exists() {
            let bytes = read_safe_file(package_path, &readme_path)?;
            update_modified(&mut modified, &readme_path);
            hash_entry(&mut hasher, "README.md", &bytes);
            Some(String::from_utf8(bytes).map_err(|error| {
                SnipError::validation(format!("{} is not UTF-8: {error}", readme_path.display()))
            })?)
        } else {
            None
        };

        let mut fragment_ids = HashSet::new();
        let mut loaded_fragments = Vec::new();
        for fragment in &manifest.fragments {
            if !fragment_ids.insert(fragment.id) {
                return Err(SnipError::validation(format!(
                    "{} has duplicate fragment UUID {}",
                    manifest_path.display(),
                    fragment.id
                )));
            }
            let content_path = resolve_managed_path(package_path, &fragment.file)?;
            let bytes = read_safe_file(package_path, &content_path)?;
            update_modified(&mut modified, &content_path);
            hash_entry(&mut hasher, &fragment.file, &bytes);
            let content = String::from_utf8(bytes).map_err(|error| {
                SnipError::validation(format!("{} is not UTF-8: {error}", content_path.display()))
            })?;
            let note_content = if let Some(note) = &fragment.note {
                let note_path = resolve_managed_path(package_path, note)?;
                let bytes = read_safe_file(package_path, &note_path)?;
                update_modified(&mut modified, &note_path);
                hash_entry(&mut hasher, note, &bytes);
                Some(String::from_utf8(bytes).map_err(|error| {
                    SnipError::validation(format!("{} is not UTF-8: {error}", note_path.display()))
                })?)
            } else {
                None
            };
            loaded_fragments.push(Fragment {
                manifest: fragment.clone(),
                content,
                note_content,
                absolute_path: content_path,
            });
        }
        let folder = package_path
            .parent()
            .and_then(|parent| parent.strip_prefix(self.snippets_dir()).ok())
            .map(path_to_slashes)
            .unwrap_or_default();
        Ok(Snippet {
            manifest,
            readme,
            folder,
            package_path: package_path.to_path_buf(),
            modified_at: modified.and_then(system_time_rfc3339),
            fingerprint: Fingerprint(hasher.finalize().to_hex().to_string()),
            loaded_fragments,
        })
    }

    pub fn resolve_snippet<'a>(
        &self,
        catalog: &'a CatalogSnapshot,
        selector: &str,
    ) -> Result<&'a Snippet> {
        let normalized_path = selector.trim_start_matches("snippets/").trim_matches('/');
        let path_matches = catalog
            .snippets
            .iter()
            .filter(|snippet| {
                snippet
                    .package_path
                    .strip_prefix(self.snippets_dir())
                    .map(path_to_slashes)
                    .is_ok_and(|path| path == normalized_path)
            })
            .collect::<Vec<_>>();
        if let [snippet] = path_matches.as_slice() {
            return Ok(snippet);
        }

        let compact = selector.replace('-', "").to_lowercase();
        if compact.len() >= 8 && compact.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let id_matches = catalog
                .snippets
                .iter()
                .filter(|snippet| {
                    snippet
                        .id
                        .simple()
                        .to_string()
                        .starts_with(compact.as_str())
                })
                .collect::<Vec<_>>();
            return one_match(id_matches, "snippet UUID", selector);
        }

        let mut title_matches = catalog
            .snippets
            .iter()
            .filter(|snippet| snippet.title == selector)
            .collect::<Vec<_>>();
        match title_matches.as_mut_slice() {
            [] => Err(SnipError::not_found(format!(
                "no snippet title matches {selector:?}"
            ))),
            [snippet] => Ok(*snippet),
            _ => {
                let snippets_dir = self.snippets_dir();
                Err(ambiguous_snippet_error(
                    &mut title_matches,
                    &snippets_dir,
                    selector,
                ))
            }
        }
    }

    pub fn find_by_folder_and_title<'a>(
        &self,
        catalog: &'a CatalogSnapshot,
        folder: &str,
        title: &str,
    ) -> Result<Option<&'a Snippet>> {
        let snippets_dir = self.snippets_dir();
        find_snippet_by_folder_and_title(&catalog.snippets, &snippets_dir, folder, title)
    }

    pub fn resolve_fragment<'a>(
        &self,
        snippet: &'a Snippet,
        selector: Option<&str>,
    ) -> Result<&'a Fragment> {
        let Some(selector) = selector else {
            return snippet.loaded_fragments.first().ok_or_else(|| {
                SnipError::validation(format!("snippet {} has no fragments", snippet.id))
            });
        };
        if let Ok(index) = selector.parse::<usize>() {
            if index == 0 {
                return Err(SnipError::usage("fragment indices start at 1"));
            }
            return snippet.loaded_fragments.get(index - 1).ok_or_else(|| {
                SnipError::not_found(format!(
                    "fragment index {index} is out of range; snippet has {} fragment(s)",
                    snippet.loaded_fragments.len()
                ))
            });
        }
        let compact = selector.replace('-', "").to_lowercase();
        if compact.len() < 8 || !compact.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(SnipError::usage(
                "fragment selector must be a 1-based index or UUID prefix of at least 8 hex digits",
            ));
        }
        let matches = snippet
            .loaded_fragments
            .iter()
            .filter(|fragment| {
                fragment
                    .id
                    .simple()
                    .to_string()
                    .starts_with(compact.as_str())
            })
            .collect::<Vec<_>>();
        one_match(matches, "fragment UUID", selector)
    }
}

fn find_snippet_by_folder_and_title<'a>(
    snippets: &'a [Snippet],
    snippets_dir: &Path,
    folder: &str,
    title: &str,
) -> Result<Option<&'a Snippet>> {
    let mut matches = snippets
        .iter()
        .filter(|snippet| snippet.folder == folder && snippet.title == title)
        .collect::<Vec<_>>();
    match matches.as_mut_slice() {
        [] => Ok(None),
        [snippet] => Ok(Some(*snippet)),
        _ => Err(ambiguous_snippet_error(&mut matches, snippets_dir, title)),
    }
}

fn ambiguous_snippet_error(
    matches: &mut Vec<&Snippet>,
    snippets_dir: &Path,
    label: &str,
) -> SnipError {
    matches.sort_by(|left, right| left.package_path.cmp(&right.package_path));
    let count = matches.len();
    let candidates = matches
        .iter()
        .take(10)
        .map(|snippet| {
            let path = snippet
                .package_path
                .strip_prefix(snippets_dir)
                .map(path_to_slashes)
                .unwrap_or_else(|_| path_to_slashes(&snippet.package_path));
            (path, snippet.folder.as_str())
        })
        .collect::<Vec<_>>();
    let width = candidates
        .iter()
        .map(|(path, _)| path.len())
        .max()
        .unwrap_or_default();
    let mut hint = String::from("pass a package path to pick one:");
    for (path, folder) in candidates {
        if folder.is_empty() {
            write!(hint, "\n  {path:<width$}  (uncategorized)")
                .expect("writing to a String cannot fail");
        } else {
            write!(hint, "\n  {path:<width$}  (folder: {folder})")
                .expect("writing to a String cannot fail");
        }
    }
    if count > 10 {
        write!(hint, "\n  … and {} more", count - 10).expect("writing to a String cannot fail");
    }
    SnipError::not_found(format!(
        "ambiguous snippet title {label:?}: {count} matches"
    ))
    .with_hint(hint)
}

fn walk_snippets(
    root: &Path,
    directory: &Path,
    packages: &mut Vec<PathBuf>,
    folders: &mut BTreeSet<String>,
) -> Result<()> {
    reject_symlink(directory)?;
    if directory.join(SNIPPET_MANIFEST).is_file() {
        packages.push(directory.to_path_buf());
        return Ok(());
    }
    if directory != root {
        folders.insert(path_to_slashes(directory.strip_prefix(root).map_err(
            |_| SnipError::validation(format!("{} is outside snippets root", directory.display())),
        )?));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|error| SnipError::io(format!("cannot read {}: {error}", directory.display())))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| {
            SnipError::io(format!("cannot enumerate {}: {error}", directory.display()))
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let file_type = entry.file_type().map_err(|error| {
            SnipError::io(format!(
                "cannot inspect {}: {error}",
                entry.path().display()
            ))
        })?;
        if file_type.is_symlink() {
            return Err(SnipError::validation(format!(
                "symbolic links are not allowed in managed snippets: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            walk_snippets(root, &entry.path(), packages, folders)?;
        }
    }
    Ok(())
}

fn update_modified(value: &mut Option<std::time::SystemTime>, path: &Path) {
    if let Ok(candidate) = fs::metadata(path).and_then(|metadata| metadata.modified())
        && value.is_none_or(|current| candidate > current)
    {
        *value = Some(candidate);
    }
}

fn one_match<'a, T>(matches: Vec<&'a T>, kind: &str, selector: &str) -> Result<&'a T> {
    match matches.as_slice() {
        [] => Err(SnipError::not_found(format!(
            "no {kind} matches {selector:?}"
        ))),
        [item] => Ok(*item),
        _ => Err(SnipError::not_found(format!(
            "ambiguous {kind} {selector:?}: {} matches",
            matches.len()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{Library, find_snippet_by_folder_and_title};
    use crate::domain::{
        CatalogSnapshot, FORMAT_NAME, Fingerprint, LibraryManifest, SCHEMA_VERSION, Snippet,
        SnippetManifest,
    };
    use crate::error::ErrorKind;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    #[test]
    fn create_lookup_matches_nested_folder_and_title() {
        let snippets = vec![
            snippet("scratch", "notes", "notes--11111111"),
            snippet("archive", "notes", "notes--22222222"),
        ];

        let found =
            find_snippet_by_folder_and_title(&snippets, Path::new("snippets"), "scratch", "notes")
                .unwrap()
                .unwrap();

        assert_eq!(
            found.package_path,
            PathBuf::from("snippets/scratch/notes--11111111")
        );
    }

    #[test]
    fn create_lookup_matches_flat_selector_with_resolved_folder() {
        let snippets = vec![snippet("default", "notes", "notes--11111111")];

        let found =
            find_snippet_by_folder_and_title(&snippets, Path::new("snippets"), "default", "notes")
                .unwrap()
                .unwrap();

        assert_eq!(found.title, "notes");
    }

    #[test]
    fn create_lookup_matches_uncategorized_empty_folder() {
        let snippets = vec![snippet("", "notes", "notes--11111111")];

        let found = find_snippet_by_folder_and_title(&snippets, Path::new("snippets"), "", "notes")
            .unwrap()
            .unwrap();

        assert!(found.folder.is_empty());
    }

    #[test]
    fn create_lookup_rejects_duplicate_folder_and_title() {
        let snippets = vec![
            snippet("scratch", "notes", "notes--11111111"),
            snippet("scratch", "notes", "notes--22222222"),
        ];

        let error =
            find_snippet_by_folder_and_title(&snippets, Path::new("snippets"), "scratch", "notes")
                .unwrap_err();

        assert_eq!(error.kind, ErrorKind::NotFound);
        assert_eq!(
            error.message,
            "ambiguous snippet title \"notes\": 2 matches"
        );
        assert_eq!(
            error.hint.as_deref(),
            Some(
                "pass a package path to pick one:\n  scratch/notes--11111111  (folder: scratch)\n  scratch/notes--22222222  (folder: scratch)"
            )
        );
    }

    #[test]
    fn create_lookup_returns_none_for_missing_snippet() {
        let snippets = vec![snippet("scratch", "other", "other--11111111")];

        let found =
            find_snippet_by_folder_and_title(&snippets, Path::new("snippets"), "scratch", "notes")
                .unwrap();

        assert!(found.is_none());
    }

    #[test]
    fn resolve_and_create_lookup_ambiguity_hints_are_byte_identical() {
        let library = library();
        let catalog = CatalogSnapshot {
            library: library.manifest.clone(),
            root: library.root.clone(),
            snippets: vec![
                snippet("scratch", "notes", "notes--11111111"),
                snippet("scratch", "notes", "notes--22222222"),
            ],
            folders: vec!["scratch".to_owned()],
            tags: Vec::new(),
        };

        let resolve_error = library.resolve_snippet(&catalog, "notes").unwrap_err();
        let create_error = find_snippet_by_folder_and_title(
            &catalog.snippets,
            Path::new("snippets"),
            "scratch",
            "notes",
        )
        .unwrap_err();

        assert_eq!(resolve_error.hint, create_error.hint);
    }

    fn library() -> Library {
        Library {
            root: PathBuf::new(),
            manifest: LibraryManifest {
                format: FORMAT_NAME.to_owned(),
                schema_version: SCHEMA_VERSION,
                id: Uuid::nil(),
                name: String::new(),
                created_at: String::new(),
                extra: toml::Table::new(),
            },
            restored: Vec::new(),
        }
    }

    fn snippet(folder: &str, title: &str, package: &str) -> Snippet {
        let package_path = if folder.is_empty() {
            PathBuf::from("snippets").join(package)
        } else {
            PathBuf::from("snippets").join(folder).join(package)
        };
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::nil(),
                title: title.to_owned(),
                tags: Vec::new(),
                pinned: false,
                locked: false,
                created_at: String::new(),
                source: None,
                remotes: Vec::new(),
                fragments: Vec::new(),
                extra: toml::Table::new(),
            },
            readme: None,
            folder: folder.to_owned(),
            package_path,
            modified_at: None,
            fingerprint: Fingerprint(String::new()),
            loaded_fragments: Vec::new(),
        }
    }
}
