use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};

use super::app::App;
use super::icons::snippet_badge;
use super::snippet_list;
use super::state::{Pane, SidebarItem, StatusLevel};
use super::theme::TuiTheme;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let vertical = if app.search.active {
        Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area)
    };
    let panes = Layout::horizontal([
        Constraint::Length(24),
        Constraint::Percentage(30),
        Constraint::Min(0),
    ])
    .split(vertical[0]);
    draw_sidebar(frame, app, panes[0]);
    draw_list(frame, app, panes[1]);
    draw_preview(frame, app, panes[2]);

    if app.search.active {
        draw_search_bar(frame, app, vertical[1]);
    }
    let context_index = usize::from(app.search.active) + 1;
    draw_context_bar(frame, app, vertical[context_index]);
    draw_command_bar(frame, app, vertical[context_index + 1]);
    if app.show_help {
        draw_help(frame, area, app.theme);
    }
}

fn draw_sidebar(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let items = app
        .sidebar
        .rows
        .iter()
        .map(|row| {
            if row.item == SidebarItem::Header {
                return ListItem::new(Line::from(Span::styled(
                    row.label.clone(),
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            let branch = if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else if matches!(row.item, SidebarItem::Tag(_)) {
                "# "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{}{}{}", "  ".repeat(row.depth), branch, row.label)),
                Span::styled(
                    format!(" {}", row.count),
                    Style::default().fg(app.theme.muted),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(pane_block("Library", app.focus == Pane::Sidebar, app.theme))
        .highlight_style(if app.focus == Pane::Sidebar {
            app.theme.selected()
        } else {
            app.theme.retained_selection()
        })
        .highlight_symbol(if app.focus == Pane::Sidebar {
            "› "
        } else {
            "· "
        });
    frame.render_stateful_widget(list, area, &mut app.sidebar.list_state);
}

fn draw_list(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let title = if app.search.query.is_empty() {
        format!("Snippets ({})", app.visible.len())
    } else {
        format!("Results ({})", app.visible.len())
    };
    let list = List::new(snippet_list::items(app))
        .block(pane_block(&title, app.focus == Pane::List, app.theme))
        .highlight_style(if app.focus == Pane::List {
            app.theme.selected()
        } else {
            app.theme.retained_selection()
        })
        .highlight_symbol(if app.focus == Pane::List {
            "› "
        } else {
            "· "
        });
    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_preview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let block = pane_block("Preview", app.focus == Pane::Preview, app.theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(snippet) = app.selected_snippet().cloned() else {
        frame.render_widget(
            Paragraph::new("No snippets match the current filter")
                .style(Style::default().fg(app.theme.muted))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };
    let regions = if snippet.loaded_fragments.len() > 1 {
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner)
    } else {
        Layout::vertical([Constraint::Length(0), Constraint::Min(0)]).split(inner)
    };
    if snippet.loaded_fragments.len() > 1 {
        let titles = snippet
            .loaded_fragments
            .iter()
            .map(|fragment| Line::from(fragment.title.clone()))
            .collect::<Vec<_>>();
        frame.render_widget(
            Tabs::new(titles)
                .select(app.fragment_index)
                .style(Style::default().fg(app.theme.muted))
                .highlight_style(app.theme.selected())
                .padding(" ", " ")
                .divider(" │ "),
            regions[0],
        );
    }
    match app
        .preview
        .get(&snippet, app.fragment_index, &app.highlighter, app.theme)
    {
        Ok(text) => {
            let width = regions[1].width.max(1) as usize;
            let rendered_rows = text
                .lines
                .iter()
                .map(|line| line.width().max(1).div_ceil(width))
                .sum::<usize>();
            let max_scroll = rendered_rows
                .saturating_sub(regions[1].height as usize)
                .min(u16::MAX as usize) as u16;
            app.preview_scroll = app.preview_scroll.min(max_scroll);
            frame.render_widget(
                Paragraph::new(text)
                    .scroll((app.preview_scroll, 0))
                    .wrap(Wrap { trim: false }),
                regions[1],
            )
        }
        Err(error) => frame.render_widget(
            Paragraph::new(error.to_string()).style(Style::default().fg(app.theme.error)),
            regions[1],
        ),
    }
}

fn draw_search_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let base = Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg);
    let line = Line::from(vec![
        Span::styled(
            " SEARCH ",
            Style::default()
                .fg(app.theme.selection_fg)
                .bg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", base),
        Span::styled(app.search.query.clone(), base),
    ]);
    frame.render_widget(Paragraph::new(line).style(base), area);
    let x = area
        .x
        .saturating_add(10 + app.search.query.chars().count() as u16)
        .min(area.right().saturating_sub(1));
    frame.set_cursor_position((x, area.y));
}

fn draw_context_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let base = Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg);
    let focus = match app.focus {
        Pane::Sidebar => "LIBRARY",
        Pane::List => "SNIPPETS",
        Pane::Preview => "PREVIEW",
    };
    let mode = if app.search.active {
        " SEARCH "
    } else {
        " NORMAL "
    };
    let filter = if let Some(folder) = &app.filter.folder {
        format!("Folder: {folder}")
    } else if let Some(tag) = &app.filter.tag {
        format!("Tag: #{tag}")
    } else {
        "All snippets".to_owned()
    };
    let context = if let Some(status) = &app.status {
        status.text.clone()
    } else if let Some(snippet) = app.selected_snippet() {
        format!(
            "{filter}  ·  [{}] {}",
            snippet_badge(snippet),
            snippet.title
        )
    } else {
        filter
    };
    let position = if let Some(index) = app.list_state.selected() {
        let fragment_count = app
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        format!(
            " {}/{}  │  fragment {}/{} ",
            index + 1,
            app.visible.len(),
            app.fragment_index.saturating_add(1).min(fragment_count),
            fragment_count
        )
    } else {
        format!(" 0/{} ", app.visible.len())
    };
    let right_width = position.chars().count().min(area.width as usize) as u16;
    let regions =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);
    let context_style = app.status.as_ref().map_or(base, |status| {
        base.fg(match status.level {
            StatusLevel::Info => app.theme.success,
            StatusLevel::Error => app.theme.error,
        })
    });
    let line = Line::from(vec![
        Span::styled(
            mode,
            Style::default()
                .fg(app.theme.selection_fg)
                .bg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {focus} "),
            Style::default()
                .fg(app.theme.selection_fg)
                .bg(app.theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", base),
        Span::styled(context, context_style),
    ]);
    frame.render_widget(Paragraph::new(line).style(base), regions[0]);
    frame.render_widget(
        Paragraph::new(position)
            .style(base.add_modifier(Modifier::BOLD))
            .alignment(Alignment::Right),
        regions[1],
    );
}

fn draw_command_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let commands: &[(&str, &str)] = if app.search.active {
        &[("Enter", "APPLY"), ("Esc", "CANCEL"), ("⌫", "DELETE")]
    } else if area.width >= 96 {
        &[
            ("Tab", "PANE"),
            ("j/k", "MOVE"),
            ("/", "SEARCH"),
            ("e", "EDIT"),
            ("y", "COPY"),
            ("?", "HELP"),
            ("q", "QUIT"),
        ]
    } else if area.width >= 68 {
        &[
            ("Tab", "PANE"),
            ("j/k", "MOVE"),
            ("/", "SEARCH"),
            ("e", "EDIT"),
            ("?", "HELP"),
            ("q", "QUIT"),
        ]
    } else {
        &[
            ("Tab", "PANE"),
            ("j/k", "MOVE"),
            ("/", "SEARCH"),
            ("q", "QUIT"),
        ]
    };
    let base = Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg);
    let mut spans = Vec::new();
    for (index, (key, action)) in commands.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("▸", base.fg(app.theme.muted)));
        }
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default()
                .fg(app.theme.selection_fg)
                .bg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {action} "), base));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
}

fn draw_help(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    let popup = centered_rect(62, 20, area);
    frame.render_widget(Clear, popup);
    let help = [
        "Navigation",
        "  Tab / Shift-Tab / h / l   change pane",
        "  j / k / arrows             move or scroll",
        "  g / G                      top / bottom",
        "",
        "Actions",
        "  /          search",
        "  Enter      apply filter / open preview",
        "  Space      expand folder",
        "  [ / ]      change fragment",
        "  e          edit in $EDITOR",
        "  y / Y      copy content / UUID",
        "  r          rescan",
        "  Esc        close / clear",
        "  q          quit",
    ]
    .join("\n");
    frame.render_widget(
        Paragraph::new(help)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn pane_block(title: &str, focused: bool, theme: TuiTheme) -> Block<'static> {
    let label = if focused {
        format!("◆ {title}")
    } else {
        title.to_owned()
    };
    Block::default()
        .title(Span::styled(
            label,
            if focused {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.border)
            },
        ))
        .borders(Borders::ALL)
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.border }))
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}
