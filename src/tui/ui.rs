use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use super::app::App;
use super::bottom_bar;
use super::help;
use super::modal;
use super::preview;
use super::selection::text_width;
use super::snippet_list;
use super::state::Pane;
use super::top_bar;
use super::trash;
use super::widgets;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);
    let panes = Layout::horizontal([
        Constraint::Length(24),
        Constraint::Percentage(30),
        Constraint::Min(0),
    ])
    .split(vertical[1]);
    app.layout.top_bar = vertical[0];
    app.layout.bottom_bar = vertical[2];
    app.layout.sidebar = panes[0];
    app.layout.list = panes[1];
    app.layout.preview = panes[2];
    app.layout.reset_tabs();
    top_bar::draw_top_bar(frame, app, vertical[0]);
    draw_sidebar(frame, app, panes[0]);
    draw_list(frame, app, panes[1]);
    preview::draw_preview(frame, app, panes[2]);

    bottom_bar::draw_bottom_bar(frame, app, vertical[2]);
    if app.show_help && app.modal.is_none() {
        help::draw_help(frame, area, app.theme);
    }
    if app.trash.open {
        trash::draw_trash(frame, app, area);
    }
    if let Some(ref mut modal) = app.modal {
        modal::draw_modal(frame, area, modal, app.theme);
    }
}

fn draw_sidebar(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let content_width = area.width.saturating_sub(3) as usize;
    let items = app
        .sidebar
        .rows
        .iter()
        .map(|row| {
            if row.item == super::state::SidebarItem::Header {
                let label = format!("{} ", row.label);
                let remaining = content_width.saturating_sub(label.chars().count());
                return ListItem::new(Line::from(vec![
                    Span::styled(label, Style::default().fg(app.theme.bar_fg)),
                    Span::styled("─".repeat(remaining), Style::default().fg(app.theme.border)),
                ]));
            }

            let (icon, icon_style, label_style) = match &row.item {
                super::state::SidebarItem::All => (
                    "≡ ",
                    Style::default().fg(app.theme.accent),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                super::state::SidebarItem::Uncategorized => (
                    "∅ ",
                    Style::default().fg(app.theme.muted),
                    Style::default().fg(app.theme.muted),
                ),
                super::state::SidebarItem::Trash => (
                    "× ",
                    Style::default().fg(app.theme.muted),
                    Style::default().fg(app.theme.muted),
                ),
                super::state::SidebarItem::Tag(_) => {
                    ("# ", Style::default().fg(app.theme.tag), Style::default())
                }
                super::state::SidebarItem::Folder(_) => {
                    let branch = if row.has_children {
                        if row.expanded { "▾ " } else { "▸ " }
                    } else {
                        "  "
                    };
                    (branch, Style::default(), Style::default())
                }
                super::state::SidebarItem::Header => unreachable!(),
            };

            let indent = if matches!(row.item, super::state::SidebarItem::Folder(_)) {
                "  ".repeat(row.depth)
            } else {
                String::new()
            };

            let count = row.count.to_string();
            let used = text_width(&indent) as usize
                + text_width(icon) as usize
                + text_width(&row.label) as usize
                + count.len();
            let padding = " ".repeat(content_width.saturating_sub(used).max(1));

            let spans = vec![
                Span::raw(indent),
                Span::styled(icon, icon_style),
                Span::styled(format!("{}{}", row.label, padding), label_style),
                Span::styled(count, Style::default().fg(app.theme.muted)),
            ];
            ListItem::new(Line::from(spans))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(widgets::pane_block(
            "Library",
            app.focus == Pane::Sidebar,
            app.theme,
        ))
        .highlight_style(if app.focus == Pane::Sidebar {
            app.theme.selected()
        } else {
            app.theme.retained_selection()
        })
        .highlight_symbol(" ");
    frame.render_stateful_widget(list, area, &mut app.sidebar.list_state);
}

fn draw_list(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = if app.search.query.is_empty() {
        format!("Snippets ({})", app.visible.len())
    } else {
        format!("Results ({})", app.visible.len())
    };
    let content_width = area.width.saturating_sub(3);
    let list = List::new(snippet_list::items(app, content_width))
        .block(widgets::pane_block(
            &title,
            app.focus == Pane::List,
            app.theme,
        ))
        .highlight_style(if app.focus == Pane::List {
            app.theme.selected()
        } else {
            Style::default()
        })
        .highlight_symbol(" ");
    frame.render_stateful_widget(list, area, &mut app.list_state);
}
