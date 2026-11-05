//! Cross-platform name diagnostics for `snip doctor`.
//!
//! Schema v1 deliberately does not make Unicode normalization, case folding, or
//! Windows device names validity requirements — that would invalidate existing
//! Unix libraries (`FORMAT.md`, "Folders"). It does recommend that tools
//! targeting cross-platform round trips avoid them, and that integrity checks
//! warn. Everything here therefore produces warnings, never errors.
//!
//! The collision domain is *siblings inside one directory*, because that is
//! what a case-insensitive or normalizing filesystem actually merges. Two
//! directories at unrelated points in the tree may share a name freely.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;

/// How two sibling names that are distinct on this filesystem would merge
/// somewhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CollisionKind {
    /// Equal under Unicode lowercase mapping: APFS, HFS+, NTFS, and exFAT
    /// treat these as one name.
    CaseInsensitive,
    /// Equal after NFC composition: macOS decomposes stored names, so a name
    /// written in NFC and one written in NFD round-trip to the same directory.
    Normalization,
}

impl CollisionKind {
    fn describe(self) -> &'static str {
        match self {
            Self::CaseInsensitive => "collide on a case-insensitive filesystem",
            Self::Normalization => "collide after Unicode NFC normalization",
        }
    }
}

/// A group of sibling names that are distinct here and identical elsewhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Collision {
    /// Slash-joined path of the containing directory, relative to `snippets/`.
    /// Empty for the uncategorized root.
    pub parent: String,
    pub kind: CollisionKind,
    /// The distinct literal spellings, sorted.
    pub names: Vec<String>,
}

/// Collects the sibling sets of a library tree and reports the groups that
/// would merge on another filesystem.
///
/// `paths` are slash-joined paths relative to `snippets/`: a folder path, or a
/// folder path with a package directory name appended. Every ancestor is
/// registered, so passing only leaf paths is enough.
pub(crate) fn collisions<I, S>(paths: I) -> Vec<Collision>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // parent -> the distinct names seen directly inside it.
    let mut siblings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in paths {
        let mut parent = String::new();
        for component in path.as_ref().split('/').filter(|part| !part.is_empty()) {
            siblings
                .entry(parent.clone())
                .or_default()
                .insert(component.to_owned());
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(component);
        }
    }

    let mut collisions = Vec::new();
    for (parent, names) in siblings {
        // A name is reported at most once per parent, under the first kind
        // that explains the clash, so `Rust` / `rust` / `Ru\u{301}st` does not
        // produce two overlapping warnings for the same pair.
        let mut reported: BTreeSet<&str> = BTreeSet::new();
        // Normalization is tested first so that a pure composition difference
        // keeps that label; the case-insensitive key composes through NFC too,
        // so it would otherwise absorb every normalization clash. What reaches
        // the second pass is a pair that differs in case, possibly in
        // composition as well.
        for kind in [CollisionKind::Normalization, CollisionKind::CaseInsensitive] {
            let mut groups: BTreeMap<String, Vec<&String>> = BTreeMap::new();
            for name in &names {
                groups.entry(fold(name, kind)).or_default().push(name);
            }
            for group in groups.into_values() {
                if group.len() < 2 || group.iter().any(|name| reported.contains(name.as_str())) {
                    continue;
                }
                for name in &group {
                    reported.insert(name.as_str());
                }
                collisions.push(Collision {
                    parent: parent.clone(),
                    kind,
                    names: group.into_iter().cloned().collect(),
                });
            }
        }
    }
    collisions
}

fn fold(name: &str, kind: CollisionKind) -> String {
    match kind {
        // Unicode lowercase mapping, the same comparison the tag registry uses
        // (`FORMAT.md`, "Tags"). Composed through NFC as well, so a pair that
        // differs in *both* case and composition is reported once, here,
        // rather than slipping past both passes.
        CollisionKind::CaseInsensitive => name.to_lowercase().nfc().collect(),
        CollisionKind::Normalization => name.nfc().collect(),
    }
}

/// Renders a collision as a `snip doctor` warning line.
///
/// A normalization clash prints identically by construction, and a mixed
/// case-and-composition clash prints as if only the case differed, so any
/// non-ASCII name is escaped to make the real difference readable — the whole
/// point of the warning is to show which names the user has to tell apart.
pub(crate) fn describe_collision(snippets_dir: &std::path::Path, collision: &Collision) -> String {
    let directory = if collision.parent.is_empty() {
        snippets_dir.display().to_string()
    } else {
        snippets_dir.join(&collision.parent).display().to_string()
    };
    let escape = collision
        .names
        .iter()
        .any(|name| !name.chars().all(|ch| ch.is_ascii_graphic() || ch == ' '));
    let names = collision
        .names
        .iter()
        .map(|name| {
            if escape {
                format!("\"{}\"", escape_non_ascii(name))
            } else {
                format!("{name:?}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{directory}: sibling names {}: {names}",
        collision.kind.describe()
    )
}

fn escape_non_ascii(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_graphic() || ch == ' ' {
                ch.to_string()
            } else {
                format!("\\u{{{:x}}}", ch as u32)
            }
        })
        .collect()
}

/// MS-DOS device names, which Windows still resolves ahead of a file of the
/// same stem in every directory. Listed in `FORMAT.md` under "Folders".
const RESERVED_DEVICE_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Reports every directory in the tree whose own name is unusable on Windows.
///
/// `paths` is the same leaf-path iterator `collisions` takes; each ancestor is
/// visited, and each offending path is reported once however many leaves pass
/// through it. The returned paths are slash-joined and relative to `snippets/`.
pub(crate) fn non_portable_paths<I, S>(paths: I) -> Vec<(String, &'static str)>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut found: BTreeMap<String, &'static str> = BTreeMap::new();
    for path in paths {
        let mut current = String::new();
        for component in path.as_ref().split('/').filter(|part| !part.is_empty()) {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(component);
            if let Some(reason) = non_portable_component(component) {
                found.insert(current.clone(), reason);
            }
        }
    }
    found.into_iter().collect()
}

/// Whether a single path component is unusable on Windows.
///
/// A device name is reserved with or without an extension, so `NUL` and
/// `NUL.txt` are both rejected; the stem is what Windows matches. Trailing dots
/// and spaces are also reported, because Windows silently strips them, which
/// turns `Notes ` into `Notes` and merges the two directories.
pub(crate) fn non_portable_component(component: &str) -> Option<&'static str> {
    let stem = component
        .split_once('.')
        .map_or(component, |(stem, _)| stem)
        .to_ascii_uppercase();
    if RESERVED_DEVICE_NAMES.contains(&stem.as_str()) {
        return Some("is a reserved device name on Windows");
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return Some("ends in a dot or space, which Windows strips");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Collision, CollisionKind, collisions, non_portable_component};

    fn find(paths: &[&str], kind: CollisionKind) -> Vec<Collision> {
        collisions(paths.iter().copied())
            .into_iter()
            .filter(|collision| collision.kind == kind)
            .collect()
    }

    #[test]
    fn distinct_sibling_names_do_not_collide() {
        assert!(collisions(["Rust/Notes", "Python/Notes", "Go"]).is_empty());
    }

    #[test]
    fn case_only_sibling_differences_collide() {
        let found = find(
            &["Rust/Notes", "rust/Other"],
            CollisionKind::CaseInsensitive,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].parent, "");
        assert_eq!(found[0].names, vec!["Rust".to_owned(), "rust".to_owned()]);
    }

    #[test]
    fn case_collisions_are_scoped_to_one_parent() {
        // `Notes` and `notes` live under different parents, so no filesystem
        // merges them.
        assert!(collisions(["Rust/Notes", "Python/notes"]).is_empty());
        let found = find(
            &["Rust/Notes", "Rust/notes"],
            CollisionKind::CaseInsensitive,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].parent, "Rust");
    }

    #[test]
    fn composition_only_sibling_differences_collide() {
        let composed = "Caf\u{e9}";
        let decomposed = "Cafe\u{301}";
        assert_ne!(composed, decomposed);
        let found = find(&[composed, decomposed], CollisionKind::Normalization);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].names.len(), 2);
    }

    #[test]
    fn a_pair_differing_in_case_and_composition_is_reported_once() {
        let found = collisions(["Caf\u{e9}", "cafe\u{301}"]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, CollisionKind::CaseInsensitive);
    }

    #[test]
    fn three_way_collisions_group_together() {
        let found = find(&["Rust", "rust", "RUST"], CollisionKind::CaseInsensitive);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].names.len(), 3);
    }

    #[test]
    fn reserved_device_names_are_reported_with_and_without_an_extension() {
        for component in ["CON", "con", "NUL.txt", "lpt9", "COM1.md", "aux"] {
            assert!(
                non_portable_component(component).is_some(),
                "{component:?} must be reported"
            );
        }
    }

    #[test]
    fn ordinary_names_are_portable() {
        for component in ["Rust", "Data Science", "你好", "CONTACTS", "com0", "lpt10"] {
            assert_eq!(
                non_portable_component(component),
                None,
                "{component:?} must be accepted"
            );
        }
    }

    #[test]
    fn trailing_dots_and_spaces_are_reported() {
        assert!(non_portable_component("Notes ").is_some());
        assert!(non_portable_component("Notes.").is_some());
        assert_eq!(non_portable_component("Notes.md"), None);
    }
}
