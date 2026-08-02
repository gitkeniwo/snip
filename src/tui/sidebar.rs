use std::collections::HashMap;

use crate::domain::CatalogSnapshot;

use super::state::{SidebarItem, SidebarRow, SidebarState};

pub fn rebuild(state: &mut SidebarState, catalog: &CatalogSnapshot, trash_count: usize) {
    let selected_key = state.selected().map(row_key);
    if state.expanded.is_empty() {
        state.expanded.extend(catalog.folders.iter().cloned());
    }

    let uncategorized_count = catalog
        .snippets
        .iter()
        .filter(|snippet| snippet.folder.is_empty())
        .count();

    let published_count = catalog
        .snippets
        .iter()
        .filter(|snippet| crate::gist::find(snippet).is_some())
        .count();

    // Three groups, in this order: the places a snippet can be (scopes), the
    // lenses you can lay over them (filters), then the folder and tag trees.
    let mut rows = vec![
        SidebarRow {
            item: SidebarItem::All,
            label: "All snippets".to_owned(),
            depth: 0,
            count: catalog.snippets.len(),
            has_children: false,
            expanded: false,
        },
        SidebarRow {
            item: SidebarItem::Uncategorized,
            label: "Uncategorized".to_owned(),
            depth: 0,
            count: uncategorized_count,
            has_children: false,
            expanded: false,
        },
        SidebarRow {
            item: SidebarItem::Trash,
            label: "Trash".to_owned(),
            depth: 0,
            count: trash_count,
            has_children: false,
            expanded: false,
        },
        SidebarRow {
            item: SidebarItem::Header,
            label: "Filters".to_owned(),
            depth: 0,
            count: 0,
            has_children: false,
            expanded: false,
        },
        SidebarRow {
            item: SidebarItem::Published,
            label: "Published".to_owned(),
            depth: 0,
            count: published_count,
            has_children: false,
            expanded: false,
        },
    ];

    if !catalog.folders.is_empty() {
        rows.push(SidebarRow {
            item: SidebarItem::Header,
            label: "Folders".to_owned(),
            depth: 0,
            count: 0,
            has_children: false,
            expanded: false,
        });

        for folder in &catalog.folders {
            if !ancestors_visible(folder, &state.expanded) {
                continue;
            }
            let prefix = format!("{folder}/");
            let has_children = catalog
                .folders
                .iter()
                .any(|candidate| candidate.starts_with(&prefix));
            let count = catalog
                .snippets
                .iter()
                .filter(|snippet| snippet.folder == *folder || snippet.folder.starts_with(&prefix))
                .count();
            rows.push(SidebarRow {
                item: SidebarItem::Folder(folder.clone()),
                label: folder.rsplit('/').next().unwrap_or(folder).to_owned(),
                depth: folder.matches('/').count(),
                count,
                has_children,
                expanded: state.expanded.contains(folder),
            });
        }
    }

    rows.push(SidebarRow {
        item: SidebarItem::Header,
        label: "Tags".to_owned(),
        depth: 0,
        count: 0,
        has_children: false,
        expanded: false,
    });
    let tag_counts = catalog
        .tags
        .iter()
        .map(|tag| {
            let count = catalog
                .snippets
                .iter()
                .filter(|snippet| {
                    snippet
                        .tags
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
                })
                .count();
            (tag.to_lowercase(), count)
        })
        .collect::<HashMap<_, _>>();
    for tag in &catalog.tags {
        rows.push(SidebarRow {
            item: SidebarItem::Tag(tag.clone()),
            label: tag.clone(),
            depth: 0,
            count: *tag_counts.get(&tag.to_lowercase()).unwrap_or(&0),
            has_children: false,
            expanded: false,
        });
    }

    state.rows = rows;
    let selection = selected_key
        .and_then(|key| state.rows.iter().position(|row| row_key(row) == key))
        .or_else(|| {
            state
                .list_state
                .selected()
                .map(|index| index.min(state.rows.len().saturating_sub(1)))
        });
    state.list_state.select(selection);
    if state.list_state.selected().is_none() {
        state.select_first_actionable();
    }
}

fn row_key(row: &SidebarRow) -> String {
    match &row.item {
        SidebarItem::All => "all".to_owned(),
        SidebarItem::Published => "published".to_owned(),
        SidebarItem::Uncategorized => "uncategorized".to_owned(),
        SidebarItem::Folder(path) => format!("folder:{path}"),
        SidebarItem::Trash => "trash".to_owned(),
        SidebarItem::Tag(tag) => format!("tag:{tag}"),
        SidebarItem::Header => "header".to_owned(),
    }
}

fn ancestors_visible(folder: &str, expanded: &std::collections::BTreeSet<String>) -> bool {
    let components = folder.split('/').collect::<Vec<_>>();
    (1..components.len()).all(|end| expanded.contains(&components[..end].join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::domain::{
        CatalogSnapshot, Fingerprint, LibraryManifest, RemoteRecord, Snippet, SnippetManifest,
    };
    use crate::filesystem::Library;
    use crate::service::{CreateOptions, create_snippet};
    use crate::tui::app::App;
    use crate::tui::command::CommandId;
    use crate::tui::state::Pane;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn make_snippet(folder: &str, gist: bool) -> Snippet {
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Test".to_owned(),
                tags: vec![],
                pinned: false,
                locked: false,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                source: None,
                remotes: gist
                    .then(|| RemoteRecord {
                        kind: "gist".to_owned(),
                        host: "github.com".to_owned(),
                        id: "5b0e0062eb8e9654adad7bb1d81cc75f".to_owned(),
                        url: "https://gist.github.com/octocat/5b0e0062eb8e9654adad7bb1d81cc75f"
                            .to_owned(),
                        public: false,
                        description: None,
                        files: vec![],
                        include_notes: false,
                        include_readme: true,
                        pushed_at: None,
                        pushed_digest: None,
                        extra: toml::Table::new(),
                    })
                    .into_iter()
                    .collect(),
                fragments: vec![],
                extra: toml::Table::new(),
            },
            readme: None,
            folder: folder.to_owned(),
            package_path: PathBuf::new(),
            modified_at: None,
            fingerprint: Fingerprint("abc".to_owned()),
            loaded_fragments: vec![],
        }
    }

    fn catalog(snippets: Vec<Snippet>) -> CatalogSnapshot {
        CatalogSnapshot {
            library: LibraryManifest {
                format: "snip-library".to_owned(),
                schema_version: 1,
                id: Uuid::new_v4(),
                name: "test".to_owned(),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                extra: toml::Table::new(),
            },
            root: PathBuf::new(),
            snippets,
            folders: vec!["Code/Rust".to_owned()],
            tags: vec![],
        }
    }

    fn app_with_folder() -> (tempfile::TempDir, App) {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Sidebar.sniplib"), None).unwrap();
        create_snippet(
            &library,
            &CreateOptions {
                title: "In Folder".to_owned(),
                folder: Some("Code/Rust".to_owned()),
                language: "rust".to_owned(),
                content: "fn main() {}\n".to_owned(),
                ..CreateOptions::default()
            },
        )
        .unwrap();
        let app = App::new(library, &AppConfig::default()).unwrap();
        (temporary, app)
    }

    fn enter(app: &mut App) {
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }

    #[test]
    fn scope_rows_lead_and_published_sits_under_its_own_filters_header() {
        let mut state = SidebarState::default();
        rebuild(
            &mut state,
            &catalog(vec![
                make_snippet("", false),
                make_snippet("Code/Rust", true),
            ]),
            0,
        );
        assert_eq!(state.rows[0].item, SidebarItem::All);
        assert_eq!(state.rows[1].item, SidebarItem::Uncategorized);
        assert_eq!(state.rows[2].item, SidebarItem::Trash);
        assert_eq!(state.rows[3].item, SidebarItem::Header);
        assert_eq!(state.rows[3].label, "Filters");
        assert_eq!(state.rows[4].item, SidebarItem::Published);
    }

    #[test]
    fn published_count_is_the_number_of_snippets_with_a_gist_record() {
        let mut state = SidebarState::default();
        rebuild(
            &mut state,
            &catalog(vec![
                make_snippet("", false),
                make_snippet("", true),
                make_snippet("Code/Rust", true),
            ]),
            0,
        );
        let published = state
            .rows
            .iter()
            .find(|row| row.item == SidebarItem::Published)
            .unwrap();
        assert_eq!(published.count, 2);
    }

    #[test]
    fn published_row_key_is_published() {
        let mut state = SidebarState::default();
        rebuild(&mut state, &catalog(vec![make_snippet("", true)]), 0);
        assert_eq!(row_key(&state.rows[4]), "published");
    }

    #[test]
    fn toggling_published_leaves_folder_and_tag_untouched() {
        let (_temporary, mut app) = app_with_folder();
        app.filter.folder = Some("Code/Rust".to_owned());
        let index = app
            .sidebar
            .rows
            .iter()
            .position(|row| row.item == SidebarItem::Published)
            .unwrap();
        app.sidebar.list_state.select(Some(index));
        app.focus = Pane::Sidebar;
        enter(&mut app);

        assert!(app.filter.published);
        assert_eq!(
            app.filter.folder.as_deref(),
            Some("Code/Rust"),
            "folder must survive the published toggle"
        );
        assert!(app.filter.tag.is_none());
    }

    #[test]
    fn selecting_a_folder_afterwards_leaves_published_on() {
        let (_temporary, mut app) = app_with_folder();
        app.filter.published = true;
        let index = app
            .sidebar
            .rows
            .iter()
            .position(|row| row.item == SidebarItem::Folder("Code/Rust".to_owned()))
            .unwrap();
        app.sidebar.list_state.select(Some(index));
        app.focus = Pane::Sidebar;
        enter(&mut app);

        assert!(
            app.filter.published,
            "published is a lens and must survive a folder scope change"
        );
        assert_eq!(app.filter.folder.as_deref(), Some("Code/Rust"));
    }

    #[test]
    fn selecting_all_snippets_leaves_published_on() {
        let (_temporary, mut app) = app_with_folder();
        app.filter.published = true;
        let index = app
            .sidebar
            .rows
            .iter()
            .position(|row| row.item == SidebarItem::All)
            .unwrap();
        app.sidebar.list_state.select(Some(index));
        app.focus = Pane::Sidebar;
        enter(&mut app);

        assert!(app.filter.published);
        assert!(app.filter.folder.is_none());
        assert!(!app.filter.uncategorized);
    }

    #[test]
    fn library_clear_filter_turns_published_off() {
        let (_temporary, mut app) = app_with_folder();
        app.filter.published = true;
        app.run_command(CommandId::LibraryClearFilter);

        assert!(!app.filter.published);
        assert!(app.filter.is_empty());
    }
}
