use std::collections::HashMap;

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use uuid::Uuid;

use crate::domain::Snippet;
use crate::gist::payload::{self, PayloadOptions};

use super::super::theme::TuiTheme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GistBadge {
    Synced,
    Modified,
}

/// Computes the badge for a single snippet, or `None` when it has no gist
/// record. Recomputation always uses the record's own `include_notes` /
/// `include_readme`, so the digest matches what was pushed.
pub fn compute(snippet: &Snippet) -> Option<GistBadge> {
    let record = crate::gist::find(snippet)?;
    let description = record
        .description
        .clone()
        .unwrap_or_else(|| snippet.title.clone());
    let payload = payload::build(
        snippet,
        &description,
        &PayloadOptions {
            include_notes: record.include_notes,
            include_readme: record.include_readme,
        },
    )
    .ok()?;
    let digest = payload::digest(&payload);
    if record.pushed_digest.as_deref() == Some(digest.as_str()) {
        Some(GistBadge::Synced)
    } else {
        Some(GistBadge::Modified)
    }
}

/// Builds the badge map for a whole catalog, keyed by snippet id. Snippets
/// without a gist record are simply absent from the map.
pub fn compute_all(snippets: &[Snippet]) -> HashMap<Uuid, GistBadge> {
    snippets
        .iter()
        .filter_map(|snippet| compute(snippet).map(|badge| (snippet.id, badge)))
        .collect()
}

/// The two spans a badge is drawn from. Both share the status colour: an accent
/// `G` next to a green tick reads as two unrelated marks in most themes, and
/// most themes make that accent blue. Always two cells total.
///
/// Deliberately not gated on `IconMode`: that setting selects away from Nerd
/// Font private-use glyphs, and `✓` is ordinary Unicode — the same class as the
/// `≡` and `∅` the sidebar already draws in either mode.
pub fn glyph(badge: GistBadge, theme: TuiTheme) -> [Span<'static>; 2] {
    let (mark, color) = match badge {
        GistBadge::Synced => ("✓", theme.success),
        GistBadge::Modified => ("+", theme.error),
    };
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    [Span::styled("G", style), Span::styled(mark, style)]
}

/// Width of a rendered badge in cells. Constant, and the row arithmetic in
/// `snippet_list` depends on it staying that way.
pub const GLYPH_WIDTH: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Fingerprint, Fragment, FragmentManifest, RemoteRecord, SnippetManifest};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fragment(name: &str, content: &str, note: Option<&str>) -> Fragment {
        Fragment {
            manifest: FragmentManifest {
                id: Uuid::new_v4(),
                title: name.to_owned(),
                language: "text".to_owned(),
                file: format!("fragments/001-{name}"),
                note: note.map(|_| format!("notes/{name}.md")),
                source_language: None,
                extra: toml::Table::new(),
            },
            content: content.to_owned(),
            note_content: note.map(str::to_owned),
            absolute_path: PathBuf::new(),
        }
    }

    fn snippet(note: Option<&str>) -> Snippet {
        let loaded_fragments = vec![fragment("Brewfile", "tap 'homebrew/bundle'\n", note)];
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

    /// Attaches a record whose pushed digest matches the snippet's current
    /// payload, so the snippet reads as `Synced` unless later edited.
    fn link(snippet: &mut Snippet, include_notes: bool) {
        let description = snippet.title.clone();
        let payload = payload::build(
            snippet,
            &description,
            &PayloadOptions {
                include_notes,
                include_readme: true,
            },
        )
        .unwrap();
        let digest = payload::digest(&payload);
        snippet.manifest.remotes.push(RemoteRecord {
            kind: "gist".to_owned(),
            host: "github.com".to_owned(),
            id: "5b0e0062eb8e9654adad7bb1d81cc75f".to_owned(),
            url: "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f".to_owned(),
            public: false,
            description: Some(description),
            files: payload.files.keys().cloned().collect(),
            include_notes,
            include_readme: true,
            pushed_at: Some("2026-08-01T10:00:00Z".to_owned()),
            pushed_digest: Some(digest),
            extra: toml::Table::new(),
        });
    }

    #[test]
    fn a_snippet_with_no_record_yields_none() {
        let snippet = snippet(None);
        assert_eq!(compute(&snippet), None);
    }

    #[test]
    fn a_matching_digest_yields_synced() {
        let mut snippet = snippet(None);
        link(&mut snippet, false);
        assert_eq!(compute(&snippet), Some(GistBadge::Synced));
    }

    #[test]
    fn editing_fragment_content_flips_the_same_snippet_to_modified() {
        let mut snippet = snippet(None);
        link(&mut snippet, false);
        snippet.loaded_fragments[0].content = "tap 'different'\n".to_owned();
        assert_eq!(compute(&snippet), Some(GistBadge::Modified));
    }

    #[test]
    fn a_record_written_with_include_notes_still_reads_synced() {
        let mut snippet = snippet(Some("Installs the bundle.\n"));
        link(&mut snippet, true);
        assert_eq!(compute(&snippet), Some(GistBadge::Synced));
    }

    #[test]
    fn both_badge_spans_share_the_status_colour() {
        let theme = TuiTheme::default_for(crate::tui::theme::Appearance::Dark);
        for (badge, mark, color) in [
            (GistBadge::Synced, "✓", theme.success),
            (GistBadge::Modified, "+", theme.error),
        ] {
            let spans = glyph(badge, theme);
            assert_eq!(spans[0].content, "G");
            assert_eq!(spans[0].style.fg, Some(color));
            assert_eq!(spans[1].content, mark);
            assert_eq!(spans[1].style.fg, Some(color));
            // The row arithmetic in snippet_list.rs assumes a constant width.
            let width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
            assert_eq!(width, GLYPH_WIDTH);
        }
    }
}
