use fs2::FileExt;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::domain::{
    CatalogSnapshot, FORMAT_NAME, Fingerprint, Fragment, FragmentManifest, LibraryManifest,
    SCHEMA_VERSION, Snippet, SnippetManifest, TagDefinition, TagRegistry,
};
use crate::error::{Result, SnipError};

const LIBRARY_MANIFEST: &str = "snip.toml";
const SNIPPET_MANIFEST: &str = "snippet.toml";

#[derive(Clone, Debug)]
pub struct Library {
    root: PathBuf,
    manifest: LibraryManifest,
}

pub struct LibraryLock {
    file: File,
}

impl Drop for LibraryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl Library {
    pub fn init(path: &Path, name: Option<&str>) -> Result<Self> {
        if path.join(LIBRARY_MANIFEST).exists() {
            return Err(SnipError::conflict(format!(
                "library already exists: {}",
                path.display()
            )));
        }
        fs::create_dir_all(path)
            .map_err(|error| SnipError::io(format!("cannot create {}: {error}", path.display())))?;
        for directory in [
            "snippets",
            "trash",
            ".snip/cache",
            ".snip/locks",
            ".snip/transactions",
        ] {
            fs::create_dir_all(path.join(directory))
                .map_err(|error| SnipError::io(format!("cannot create {directory}: {error}")))?;
        }
        let inferred_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Snippets");
        let manifest = LibraryManifest {
            format: FORMAT_NAME.to_owned(),
            schema_version: SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: name.unwrap_or(inferred_name).to_owned(),
            created_at: now_rfc3339()?,
            extra: toml::Table::new(),
        };
        let data = toml::to_string_pretty(&manifest)?;
        atomic_write(&path.join(LIBRARY_MANIFEST), data.as_bytes())?;
        let tags = TagRegistry {
            schema_version: SCHEMA_VERSION,
            tags: Vec::new(),
            extra: toml::Table::new(),
        };
        atomic_write(
            &path.join("tags.toml"),
            toml::to_string_pretty(&tags)?.as_bytes(),
        )?;
        let ignore = ".snip/cache/\n.snip/locks/\n.snip/transactions/\n.DS_Store\n";
        atomic_write(&path.join(".gitignore"), ignore.as_bytes())?;
        Self::open(path)
    }

    pub fn discover(explicit: Option<&Path>, configured: Option<&Path>) -> Result<PathBuf> {
        if let Some(path) = explicit {
            return Ok(path.to_path_buf());
        }
        if let Some(path) = std::env::var_os("SNIP_LIBRARY") {
            return Ok(PathBuf::from(path));
        }
        let current = std::env::current_dir()
            .map_err(|error| SnipError::io(format!("cannot get current directory: {error}")))?;
        for candidate in current.ancestors() {
            if candidate.join(LIBRARY_MANIFEST).is_file() {
                return Ok(candidate.to_path_buf());
            }
        }
        if let Some(path) = configured {
            return Ok(path.to_path_buf());
        }
        Err(SnipError::not_found(
            "no snip library found; pass --library, set SNIP_LIBRARY, run inside a library, or configure default_library",
        ))
    }

    pub fn open(path: &Path) -> Result<Self> {
        let root = fs::canonicalize(path).map_err(|error| {
            SnipError::not_found(format!(
                "library does not exist: {}: {error}",
                path.display()
            ))
        })?;
        let manifest_path = root.join(LIBRARY_MANIFEST);
        let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| {
            SnipError::validation(format!("cannot read {}: {error}", manifest_path.display()))
        })?;
        let manifest: LibraryManifest = toml::from_str(&manifest_text).map_err(|error| {
            SnipError::validation(format!("cannot parse {}: {error}", manifest_path.display()))
        })?;
        if manifest.format != FORMAT_NAME {
            return Err(SnipError::validation(format!(
                "{} has unsupported format {:?}",
                manifest_path.display(),
                manifest.format
            )));
        }
        validate_schema(manifest.schema_version, &manifest_path)?;
        for required in ["snippets", "trash", ".snip"] {
            if !root.join(required).is_dir() {
                return Err(SnipError::validation(format!(
                    "library is missing directory: {}",
                    root.join(required).display()
                )));
            }
        }
        Ok(Self { root, manifest })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &LibraryManifest {
        &self.manifest
    }

    pub fn snippets_dir(&self) -> PathBuf {
        self.root.join("snippets")
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.root.join("trash")
    }

    pub fn transactions_dir(&self) -> PathBuf {
        self.root.join(".snip/transactions")
    }

    pub fn tag_registry(&self) -> Result<TagRegistry> {
        let path = self.root.join("tags.toml");
        if !path.exists() {
            return Ok(TagRegistry {
                schema_version: SCHEMA_VERSION,
                tags: Vec::new(),
                extra: toml::Table::new(),
            });
        }
        let registry: TagRegistry =
            toml::from_str(&fs::read_to_string(&path).map_err(|error| {
                SnipError::validation(format!("cannot read {}: {error}", path.display()))
            })?)
            .map_err(|error| {
                SnipError::validation(format!("cannot parse {}: {error}", path.display()))
            })?;
        validate_schema(registry.schema_version, &path)?;
        let mut seen_names = HashSet::new();
        let mut seen_ids = HashSet::new();
        for tag in &registry.tags {
            if tag.name.trim().is_empty() {
                return Err(SnipError::validation(format!(
                    "{} contains an empty tag name",
                    path.display()
                )));
            }
            if !seen_names.insert(tag.name.trim().to_lowercase()) {
                return Err(SnipError::validation(format!(
                    "{} contains duplicate tag name {:?}",
                    path.display(),
                    tag.name
                )));
            }
            if !seen_ids.insert(tag.id) {
                return Err(SnipError::validation(format!(
                    "{} contains duplicate tag UUID {}",
                    path.display(),
                    tag.id
                )));
            }
        }
        Ok(registry)
    }

    pub fn write_tag_registry(&self, registry: &TagRegistry) -> Result<()> {
        let data = toml::to_string_pretty(registry)?;
        atomic_write(&self.root.join("tags.toml"), data.as_bytes())
    }

    pub fn register_tags(&self, names: &[String]) -> Result<()> {
        let normalized = normalize_tags(names)?;
        let mut registry = self.tag_registry()?;
        let known = registry
            .tags
            .iter()
            .map(|tag| tag.name.to_lowercase())
            .collect::<HashSet<_>>();
        let mut changed = false;
        for name in normalized {
            if !known.contains(&name.to_lowercase()) {
                registry.tags.push(TagDefinition {
                    id: Uuid::new_v4(),
                    name,
                    color: None,
                    source_id: None,
                    extra: toml::Table::new(),
                });
                changed = true;
            }
        }
        if changed {
            registry
                .tags
                .sort_by_key(|left| left.name.to_lowercase());
            self.write_tag_registry(&registry)?;
        }
        Ok(())
    }

    pub fn lock(&self) -> Result<LibraryLock> {
        let lock_path = self.root.join(".snip/locks/library.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| {
                SnipError::io(format!("cannot open {}: {error}", lock_path.display()))
            })?;
        file.try_lock_exclusive().map_err(|error| {
            SnipError::conflict(format!("library is locked by another process: {error}"))
        })?;
        Ok(LibraryLock { file })
    }

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
        reject_symlink(package_path)?;
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

        let title_matches = catalog
            .snippets
            .iter()
            .filter(|snippet| snippet.title == selector)
            .collect::<Vec<_>>();
        one_match(title_matches, "snippet title", selector)
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

pub fn now_rfc3339() -> Result<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| SnipError::io(format!("cannot format current time: {error}")))
}

pub fn normalize_tags(tags: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for raw in tags {
        let tag = raw.trim();
        if tag.is_empty() {
            return Err(SnipError::validation("tags cannot be empty"));
        }
        let key = tag.to_lowercase();
        if seen.insert(key) {
            normalized.push(tag.to_owned());
        }
    }
    Ok(normalized)
}

pub fn safe_component(value: &str) -> String {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in value.trim().chars() {
        let replace = ch.is_control() || matches!(ch, '/' | '\\' | ':');
        if replace {
            if !previous_dash {
                result.push('-');
                previous_dash = true;
            }
        } else {
            result.push(ch);
            previous_dash = ch == '-';
        }
        if result.len() >= 80 {
            while !result.is_char_boundary(result.len()) {
                result.pop();
            }
            break;
        }
    }
    let result = result.trim_matches([' ', '.', '-']).to_owned();
    if result.is_empty() || result == "." || result == ".." {
        "untitled".to_owned()
    } else {
        result
    }
}

pub fn package_name(title: &str, id: Uuid) -> String {
    format!(
        "{}--{}",
        safe_component(title),
        &id.simple().to_string()[..8]
    )
}

pub fn fragment_relative_path(index: usize, title: &str, language: &str) -> String {
    let mut name = safe_component(title);
    if !name.contains('.')
        && !is_special_filename(&name)
        && let Some(extension) = extension_for_language(language)
    {
        name.push('.');
        name.push_str(extension);
    }
    format!("fragments/{index:03}-{name}")
}

pub fn note_relative_path(index: usize) -> String {
    format!("notes/{index:03}.md")
}

pub fn extension_for_language(language: &str) -> Option<&'static str> {
    match language.to_ascii_lowercase().as_str() {
        "bash" | "shell" | "sh" => Some("sh"),
        "fish" => Some("fish"),
        "python" => Some("py"),
        "rust" => Some("rs"),
        "javascript" | "js" => Some("js"),
        "typescript" | "ts" => Some("ts"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "markdown" | "md" => Some("md"),
        "html" => Some("html"),
        "css" => Some("css"),
        "sql" => Some("sql"),
        "go" | "golang" => Some("go"),
        "tcl" => Some("tcl"),
        "dockerfile" | "makefile" | "text" | "plain" => None,
        _ => None,
    }
}

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| SnipError::validation(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|error| SnipError::io(format!("cannot create {}: {error}", parent.display())))?;
    let permissions = fs::metadata(path).ok().map(|value| value.permissions());
    let mut temp = NamedTempFile::new_in(parent).map_err(|error| {
        SnipError::io(format!(
            "cannot create temporary file in {}: {error}",
            parent.display()
        ))
    })?;
    temp.write_all(data).map_err(|error| {
        SnipError::io(format!(
            "cannot write temporary file for {}: {error}",
            path.display()
        ))
    })?;
    temp.as_file().sync_all().map_err(|error| {
        SnipError::io(format!(
            "cannot sync temporary file for {}: {error}",
            path.display()
        ))
    })?;
    if let Some(permissions) = permissions {
        temp.as_file()
            .set_permissions(permissions)
            .map_err(|error| {
                SnipError::io(format!(
                    "cannot preserve permissions for {}: {error}",
                    path.display()
                ))
            })?;
    }
    temp.persist(path).map_err(|error| {
        SnipError::io(format!(
            "cannot replace {}: {}",
            path.display(),
            error.error
        ))
    })?;
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub fn write_snippet_manifest(path: &Path, manifest: &SnippetManifest) -> Result<()> {
    let data = toml::to_string_pretty(manifest)?;
    atomic_write(path, data.as_bytes())
}

pub fn resolve_managed_path(package: &Path, relative: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(SnipError::validation(format!(
            "managed path must stay inside the snippet package: {relative:?}"
        )));
    }
    Ok(package.join(relative_path))
}

fn validate_schema(version: u32, path: &Path) -> Result<()> {
    if version > SCHEMA_VERSION {
        return Err(SnipError::validation(format!(
            "{} uses schema version {version}, but this snip supports up to {SCHEMA_VERSION}",
            path.display()
        )));
    }
    if version == 0 {
        return Err(SnipError::validation(format!(
            "{} has invalid schema version 0",
            path.display()
        )));
    }
    Ok(())
}

fn validate_snippet_manifest(manifest: &SnippetManifest, path: &Path) -> Result<()> {
    validate_schema(manifest.schema_version, path)?;
    if manifest.title.trim().is_empty() {
        return Err(SnipError::validation(format!(
            "{} has an empty title",
            path.display()
        )));
    }
    if manifest.fragments.is_empty() {
        return Err(SnipError::validation(format!(
            "{} must contain at least one fragment",
            path.display()
        )));
    }
    normalize_tags(&manifest.tags)?;
    for fragment in &manifest.fragments {
        validate_fragment(fragment, path)?;
    }
    Ok(())
}

fn validate_fragment(fragment: &FragmentManifest, manifest_path: &Path) -> Result<()> {
    if fragment.title.trim().is_empty() {
        return Err(SnipError::validation(format!(
            "{} has a fragment with an empty title",
            manifest_path.display()
        )));
    }
    if fragment.language.trim().is_empty() {
        return Err(SnipError::validation(format!(
            "{} has a fragment with an empty language",
            manifest_path.display()
        )));
    }
    let package = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    resolve_managed_path(package, &fragment.file)?;
    if let Some(note) = &fragment.note {
        resolve_managed_path(package, note)?;
    }
    Ok(())
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

fn read_safe_file(package: &Path, path: &Path) -> Result<Vec<u8>> {
    reject_symlink(path)?;
    let canonical_package = fs::canonicalize(package).map_err(|error| {
        SnipError::validation(format!("cannot resolve {}: {error}", package.display()))
    })?;
    let canonical_path = fs::canonicalize(path).map_err(|error| {
        SnipError::validation(format!("cannot resolve {}: {error}", path.display()))
    })?;
    if !canonical_path.starts_with(&canonical_package) {
        return Err(SnipError::validation(format!(
            "managed file escapes snippet package: {}",
            path.display()
        )));
    }
    fs::read(path)
        .map_err(|error| SnipError::validation(format!("cannot read {}: {error}", path.display())))
}

fn reject_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        SnipError::validation(format!("cannot inspect {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(SnipError::validation(format!(
            "symbolic links are not allowed in managed paths: {}",
            path.display()
        )));
    }
    Ok(())
}

fn hash_entry(hasher: &mut blake3::Hasher, name: &str, data: &[u8]) {
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&(data.len() as u64).to_le_bytes());
    hasher.update(data);
}

fn update_modified(value: &mut Option<std::time::SystemTime>, path: &Path) {
    if let Ok(candidate) = fs::metadata(path).and_then(|metadata| metadata.modified())
        && value.is_none_or(|current| candidate > current)
    {
        *value = Some(candidate);
    }
}

fn system_time_rfc3339(value: std::time::SystemTime) -> Option<String> {
    OffsetDateTime::from(value).format(&Rfc3339).ok()
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

fn path_to_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn is_special_filename(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "brewfile" | "dockerfile" | "makefile" | "justfile" | "procfile"
    )
}
