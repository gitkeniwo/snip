use std::collections::HashMap;

use crate::domain::CatalogSnapshot;

use super::state::{SidebarItem, SidebarRow, SidebarState};

pub fn rebuild(state: &mut SidebarState, catalog: &CatalogSnapshot) {
    let selected_key = state.selected().map(row_key);
    if state.expanded.is_empty() {
        state.expanded.extend(catalog.folders.iter().cloned());
    }

    let mut rows = vec![SidebarRow {
        item: SidebarItem::All,
        label: "All snippets".to_owned(),
        depth: 0,
        count: catalog.snippets.len(),
        has_children: false,
        expanded: false,
    }];

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
        SidebarItem::Folder(path) => format!("folder:{path}"),
        SidebarItem::Tag(tag) => format!("tag:{tag}"),
        SidebarItem::Header => "header".to_owned(),
    }
}

fn ancestors_visible(folder: &str, expanded: &std::collections::BTreeSet<String>) -> bool {
    let components = folder.split('/').collect::<Vec<_>>();
    (1..components.len()).all(|end| expanded.contains(&components[..end].join("/")))
}
