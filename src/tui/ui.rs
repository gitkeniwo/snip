use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};

use super::app::App;
use super::modal::Modal;
use super::preview::PreviewDocument;
use super::selection::{SelectionKey, SelectionRow, char_width, text_width};
use super::snippet_list;
use super::state::{Pane, SidebarItem, StatusLevel};
use super::theme::TuiTheme;

type Shortcut<'a> = (&'a str, &'a str);
type ShortcutSet<'a> = &'a [Shortcut<'a>];

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
    draw_top_bar(frame, app, vertical[0]);
    draw_sidebar(frame, app, panes[0]);
    draw_list(frame, app, panes[1]);
    draw_preview(frame, app, panes[2]);

    draw_bottom_bar(frame, app, vertical[2]);
    if app.show_help && app.modal.is_none() {
        draw_help(frame, area, app.theme);
    }
    if app.trash.open {
        draw_trash(frame, app, area);
    }
    if app.modal.is_some() {
        draw_modal(frame, app, area);
    }
}

fn draw_sidebar(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    // One selection cell mirrors the block title's leading padding, so the
    // folder caret, Tags rule, and tag `#` align with the `L` in `Library`.
    let content_width = area.width.saturating_sub(3) as usize;
    let items = app
        .sidebar
        .rows
        .iter()
        .map(|row| {
            if row.item == SidebarItem::Header {
                // Tags is a section label, not a tree item: start it at the
                // same content edge as the folder caret and tag marker.
                let label = format!("{} ", row.label);
                let remaining = content_width.saturating_sub(label.chars().count());
                return ListItem::new(Line::from(vec![
                    Span::styled(label, Style::default().fg(app.theme.tag)),
                    Span::styled("─".repeat(remaining), Style::default().fg(app.theme.border)),
                ]));
            }
            let branch = if matches!(row.item, SidebarItem::Tag(_)) {
                ""
            } else if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let prefix = format!("{}{}", "  ".repeat(row.depth), branch);
            let count = row.count.to_string();
            let used = prefix.chars().count()
                + row.label.chars().count()
                + count.len()
                + 2 * usize::from(matches!(row.item, SidebarItem::Tag(_)));
            let padding = " ".repeat(content_width.saturating_sub(used).max(1));
            let mut spans = vec![Span::raw(prefix)];
            if matches!(row.item, SidebarItem::Tag(_)) {
                spans.push(Span::styled("# ", Style::default().fg(app.theme.tag)));
                spans.push(Span::raw(format!("{}{}", row.label, padding)));
            } else {
                spans.push(Span::raw(format!("{}{}", row.label, padding)));
            }
            spans.push(Span::styled(count, Style::default().fg(app.theme.muted)));
            ListItem::new(Line::from(spans))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(pane_block("Library", app.focus == Pane::Sidebar, app.theme))
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
    // One highlight-symbol cell aligns item content with the first character
    // of the block title (`Snippets`) instead of its leading title padding.
    let content_width = area.width.saturating_sub(3);
    let list = List::new(snippet_list::items(app, content_width))
        .block(pane_block(&title, app.focus == Pane::List, app.theme))
        .highlight_style(if app.focus == Pane::List {
            app.theme.selected()
        } else {
            // A retained snippet selection is styled on its title line in
            // `snippet_list`; applying it here would fill both item rows.
            Style::default()
        })
        // The selected background is enough to identify the row. A visible
        // glyph here inherits selection_fg and looks like a stray white rule.
        .highlight_symbol(" ");
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
    let has_tags = !snippet.tags.is_empty();
    let regions = if has_tags {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner)
    } else {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner)
    };
    let title_area = regions[0];
    let metadata_area = regions[1];
    let tags_area = has_tags.then_some(regions[2]);
    let rule_area = if has_tags { regions[3] } else { regions[2] };
    let raw_content_area = if has_tags { regions[4] } else { regions[3] };
    app.layout.preview_tabs = metadata_area;
    let content_area = if app.show_line_numbers {
        raw_content_area
    } else {
        inset_left(raw_content_area, 1)
    };
    app.layout.preview_content = content_area;
    draw_preview_header(
        frame,
        app,
        &snippet,
        title_area,
        metadata_area,
        tags_area,
        rule_area,
    );
    match app
        .preview
        .get(&snippet, app.fragment_index, &app.highlighter, app.theme)
    {
        Ok(document) => {
            let text = compose_preview(
                document,
                app.show_line_numbers,
                app.theme,
                content_area.width.max(1),
            );
            let rendered = wrap_preview(text, content_area.width.max(1), app.show_line_numbers);
            app.preview_selection.prepare(
                SelectionKey {
                    snippet_id: snippet.id,
                    fragment_index: app.fragment_index,
                    fingerprint: snippet.fingerprint.0.clone(),
                },
                rendered.rows,
            );
            let max_scroll = rendered
                .text
                .lines
                .len()
                .saturating_sub(content_area.height as usize)
                .min(u16::MAX as usize) as u16;
            app.preview_scroll = app.preview_scroll.min(max_scroll);
            frame.render_widget(
                Paragraph::new(rendered.text).scroll((app.preview_scroll, 0)),
                content_area,
            );
            draw_preview_selection(frame, app, content_area);
        }
        Err(error) => {
            app.preview_selection.clear();
            frame.render_widget(
                Paragraph::new(error.to_string()).style(Style::default().fg(app.theme.error)),
                content_area,
            );
        }
    }
}

fn draw_top_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let base = Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg);
    let position = if let Some(index) = app.list_state.selected() {
        let fragments = app
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        let position = format!(
            " {}/{} · {}/{} ",
            index + 1,
            app.visible.len(),
            app.fragment_index.saturating_add(1).min(fragments),
            fragments
        );
        if let Some(sort) = app.sort.indicator() {
            format!(" {sort}  {position}")
        } else {
            position
        }
    } else {
        format!(" 0/{} ", app.visible.len())
    };
    let brand_color = if app.modal.is_some() {
        app.theme.accent_alt
    } else if app.search.active {
        app.theme.warning
    } else {
        app.theme.accent
    };
    let right_width = position.chars().count().min(area.width as usize) as u16;
    let regions = Layout::horizontal([
        Constraint::Length(6),
        Constraint::Min(0),
        Constraint::Length(right_width),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            " snip ",
            Style::default()
                .fg(app.theme.selection_fg)
                .bg(brand_color)
                .add_modifier(Modifier::BOLD),
        )),
        regions[0],
    );
    let mut breadcrumb = vec![Span::styled("  ", base)];
    breadcrumb.extend(breadcrumb_spans(
        app,
        regions[1].width.saturating_sub(2) as usize,
        base,
    ));
    let line = Line::from(breadcrumb);
    frame.render_widget(Paragraph::new(line).style(base), regions[1]);
    frame.render_widget(
        Paragraph::new(position)
            .style(base.add_modifier(Modifier::BOLD))
            .alignment(Alignment::Right),
        regions[2],
    );
}

fn draw_bottom_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if let Some(modal) = &app.modal {
        match modal {
            Modal::Input(input) => {
                let prefix = format!("{}: ", input.label);
                let mut spans = vec![
                    Span::styled(
                        prefix.clone(),
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(input.value.clone()),
                ];
                if let Some(error) = &input.error {
                    spans.push(Span::styled("  ● ", Style::default().fg(app.theme.error)));
                    spans.push(Span::styled(
                        error.clone(),
                        Style::default().fg(app.theme.error),
                    ));
                }
                frame.render_widget(Paragraph::new(Line::from(spans)), area);
                let before_cursor = input.value.chars().take(input.cursor).count() as u16;
                let x = area
                    .x
                    .saturating_add(prefix.chars().count() as u16 + before_cursor)
                    .min(area.right().saturating_sub(1));
                frame.set_cursor_position((x, area.y));
            }
            Modal::Confirm(_) => frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        "y/Enter",
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" confirm  ", Style::default().fg(app.theme.muted)),
                    Span::styled(
                        "n/Esc",
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" cancel", Style::default().fg(app.theme.muted)),
                ])),
                area,
            ),
            Modal::Picker(picker) => frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("/ ", Style::default().fg(app.theme.accent)),
                    Span::raw(picker.filter.clone()),
                    Span::styled(
                        "  Enter select  Esc cancel",
                        Style::default().fg(app.theme.muted),
                    ),
                ])),
                area,
            ),
        }
        return;
    }
    if app.search.active {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "/ ",
                    Style::default()
                        .fg(app.theme.warning)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(app.search.query.clone()),
            ])),
            area,
        );
        let x = area
            .x
            .saturating_add(2 + app.search.query.chars().count() as u16)
            .min(area.right().saturating_sub(1));
        frame.set_cursor_position((x, area.y));
        return;
    }
    if let Some(status) = &app.status {
        let color = match status.level {
            StatusLevel::Info => app.theme.success,
            StatusLevel::Error => app.theme.error,
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(color)),
                Span::raw(status.text.clone()),
            ])),
            area,
        );
        return;
    }
    let navigation_full: ShortcutSet<'_> = &[
        ("←/→", "nav"),
        ("Tab", "pane"),
        ("/", "search"),
        ("?", "help"),
        ("q", "quit"),
    ];
    let navigation_medium: ShortcutSet<'_> = &[
        ("←/→", "nav"),
        ("Tab", "pane"),
        ("/", "search"),
        ("?", "help"),
    ];
    let navigation_compact: ShortcutSet<'_> = &[("←/→", ""), ("Tab", ""), ("/", ""), ("?", "")];
    let navigation_minimal: ShortcutSet<'_> = &[("Tab", ""), ("/", "")];

    let (actions_full, actions_medium, actions_compact): (
        ShortcutSet<'_>,
        ShortcutSet<'_>,
        ShortcutSet<'_>,
    ) = if app.trash.open {
        (
            &[("j/k", "move"), ("u", "restore"), ("x", "purge")],
            &[("u", "restore"), ("x", "purge")],
            &[("u", ""), ("x", "")],
        )
    } else {
        match app.focus {
            Pane::Sidebar => (
                &[
                    ("n", "new"),
                    ("r", "rename"),
                    ("d", "delete"),
                    ("s", "sort"),
                ],
                &[("n", "new"), ("r", "rename"), ("d", "delete")],
                &[("n", ""), ("r", ""), ("d", "")],
            ),
            Pane::List => (
                &[
                    ("n", "new"),
                    ("e", "edit"),
                    ("r", "rename"),
                    ("m", "move"),
                    ("t", "tags"),
                    ("d", "trash"),
                ],
                &[("n", "new"), ("e", "edit"), ("d", "trash")],
                &[("n", ""), ("e", ""), ("d", "")],
            ),
            Pane::Preview => (
                &[
                    ("e", "code"),
                    ("E", "note"),
                    ("R", "readme"),
                    ("N", "lines"),
                    ("y", "copy"),
                ],
                &[("e", "edit"), ("N", "lines"), ("y", "copy")],
                &[("e", ""), ("N", ""), ("y", "")],
            ),
        }
    };

    let tiers = [
        (navigation_full, actions_full),
        (navigation_medium, actions_medium),
        (navigation_compact, actions_medium),
        (navigation_compact, actions_compact),
        (navigation_minimal, &actions_compact[..1]),
    ];
    let (navigation, actions) = tiers
        .into_iter()
        .find(|(navigation, actions)| {
            shortcut_pills_width(navigation) + shortcut_pills_width(actions) + 2
                <= area.width as usize
        })
        .unwrap_or((navigation_minimal, &actions_compact[..1]));

    let left = shortcut_pills(
        navigation,
        app.theme.accent,
        app.theme.retained_bg,
        app.theme,
    );
    let right = shortcut_pills(actions, app.theme.accent_alt, app.theme.bar_bg, app.theme);
    let right_width = right.width().min(area.width as usize) as u16;
    let regions =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).split(area);
    frame.render_widget(Paragraph::new(left), regions[0]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        regions[1],
    );
}

fn shortcut_pills(
    commands: ShortcutSet<'_>,
    key_color: ratatui::style::Color,
    background: ratatui::style::Color,
    theme: TuiTheme,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, action)) in commands.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        // Standard Unicode half-circles form a capsule without requiring a
        // Powerline or Nerd Font glyph.
        spans.push(Span::styled("◗", Style::default().fg(background)));
        spans.push(Span::styled(
            format!(" {key}"),
            Style::default()
                .fg(key_color)
                .bg(background)
                .add_modifier(Modifier::BOLD),
        ));
        if action.is_empty() {
            spans.push(Span::styled(" ", Style::default().bg(background)));
        } else {
            spans.push(Span::styled(
                format!(" {action} "),
                Style::default().fg(theme.muted).bg(background),
            ));
        }
        spans.push(Span::styled("◖", Style::default().fg(background)));
    }
    Line::from(spans)
}

fn shortcut_pills_width(commands: ShortcutSet<'_>) -> usize {
    commands
        .iter()
        .map(|(key, action)| {
            // Two cap cells, two inner spaces, and an optional separator plus
            // action label.
            4 + text_width(key) as usize
                + if action.is_empty() {
                    0
                } else {
                    1 + text_width(action) as usize
                }
        })
        .sum::<usize>()
        + commands.len().saturating_sub(1)
}

fn draw_modal(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let theme = app.theme;
    let Some(modal) = app.modal.as_mut() else {
        return;
    };
    match modal {
        Modal::Input(_) => {}
        Modal::Confirm(confirm) => {
            let popup = centered_rect(62, 8, area);
            frame.render_widget(Clear, popup);
            let border = if confirm.destructive {
                theme.error
            } else {
                theme.accent
            };
            let mut lines = vec![Line::from(confirm.message.clone()), Line::default()];
            if let Some(error) = &confirm.error {
                lines.push(Line::from(Span::styled(
                    error.clone(),
                    Style::default().fg(theme.error),
                )));
            }
            frame.render_widget(
                Paragraph::new(lines)
                    .block(
                        Block::default()
                            .title(format!(" {} ", confirm.title))
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(border)),
                    )
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }
        Modal::Picker(picker) => {
            let popup = centered_rect(62, 18, area);
            frame.render_widget(Clear, popup);
            let filtered = picker.filtered();
            let items = filtered
                .iter()
                .map(|item| ListItem::new((*item).to_owned()))
                .collect::<Vec<_>>();
            let mut state = ratatui::widgets::ListState::default();
            state.select((!items.is_empty()).then_some(picker.selected));
            frame.render_stateful_widget(
                List::new(items)
                    .block(
                        Block::default()
                            .title(format!(" {} ", picker.label))
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(theme.accent)),
                    )
                    .highlight_symbol("▌ ")
                    .highlight_style(theme.selected()),
                popup,
                &mut state,
            );
            if let Some(error) = &picker.error {
                let error_area = Rect {
                    x: popup.x.saturating_add(2),
                    y: popup.bottom().saturating_sub(2),
                    width: popup.width.saturating_sub(4),
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(error.clone()).style(Style::default().fg(theme.error)),
                    error_area,
                );
            }
        }
    }
}

fn draw_trash(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(76, 20, area);
    frame.render_widget(Clear, popup);
    let items = app
        .trash
        .entries
        .iter()
        .map(|entry| {
            ListItem::new(vec![
                Line::from(Span::styled(
                    entry.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(vec![
                    Span::styled(
                        entry.original_path.clone(),
                        Style::default().fg(app.theme.muted),
                    ),
                    Span::styled("  ·  ", Style::default().fg(app.theme.rule)),
                    Span::styled(
                        entry.deleted_at.clone(),
                        Style::default().fg(app.theme.muted),
                    ),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let mut state = ratatui::widgets::ListState::default();
    state.select((!items.is_empty()).then_some(app.trash.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title(format!(" Trash ({}) ", app.trash.entries.len()))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(app.theme.accent_alt)),
            )
            .highlight_symbol("▌ ")
            .highlight_style(app.theme.selected()),
        popup,
        &mut state,
    );
}

fn draw_preview_header(
    frame: &mut Frame<'_>,
    app: &mut App,
    snippet: &crate::domain::Snippet,
    title_area: Rect,
    metadata_area: Rect,
    tags_area: Option<Rect>,
    rule_area: Rect,
) {
    let title_area = inset_left(title_area, 1);
    let metadata_area = inset_left(metadata_area, 1);
    let tags_area = tags_area.map(|area| inset_left(area, 1));
    let rule_area = inset_left(rule_area, 1);
    let marker = match (snippet.pinned, snippet.locked) {
        (true, true) => "★ pinned · ⊘ locked",
        (true, false) => "★ pinned",
        (false, true) => "⊘ locked",
        (false, false) => "",
    };
    let title_width = title_area
        .width
        .saturating_sub(marker.chars().count() as u16 + 2) as usize;
    let title = truncate_end(&snippet.title, title_width);
    let padding = " ".repeat(
        title_area
            .width
            .saturating_sub(title.chars().count() as u16 + marker.chars().count() as u16 + 3)
            as usize,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(padding),
            Span::styled(marker.to_owned(), Style::default().fg(app.theme.warning)),
            // `★` is treated as an ambiguous-width glyph by some terminals.
            // Keep a safety margin so the final characters never hit the border.
            Span::raw("   "),
        ])),
        title_area,
    );
    let mut metadata = vec![Span::styled(
        if snippet.folder.is_empty() {
            "~".to_owned()
        } else {
            snippet.folder.clone()
        },
        Style::default().fg(app.theme.muted),
    )];
    let multiple_fragments = snippet.loaded_fragments.len() > 1;
    if !multiple_fragments {
        metadata.push(Span::styled(" · ", Style::default().fg(app.theme.muted)));
        metadata.push(Span::styled(
            snippet.fingerprint.0.chars().take(8).collect::<String>(),
            Style::default().fg(app.theme.muted),
        ));
    }

    let mut start = metadata_area
        .x
        .saturating_add(Line::from(metadata.clone()).width() as u16);
    for (index, fragment) in snippet.loaded_fragments.iter().take(16).enumerate() {
        let separator = if index == 0 { " · " } else { " │ " };
        metadata.push(Span::styled(separator, Style::default().fg(app.theme.rule)));
        start = start.saturating_add(separator.chars().count() as u16);
        let file = std::path::Path::new(&fragment.file)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&fragment.title);
        let label = if multiple_fragments {
            file.to_owned()
        } else {
            format!(
                "{file} {}",
                super::icons::language_badge(&fragment.language)
            )
        };
        let available = metadata_area.right().saturating_sub(start) as usize;
        let label = truncate_end(&label, available);
        let width = Line::raw(label.clone()).width() as u16;
        app.layout.tab_spans[index] = (start, start.saturating_add(width));
        app.layout.tab_count += 1;
        metadata.push(Span::styled(
            label,
            if index == app.fragment_index {
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.muted)
            },
        ));
        start = start.saturating_add(width);
        if start >= metadata_area.right() {
            break;
        }
    }
    frame.render_widget(Paragraph::new(Line::from(metadata)), metadata_area);

    if let Some(tags_area) = tags_area {
        let mut tags = Vec::new();
        for tag in &snippet.tags {
            if !tags.is_empty() {
                tags.push(Span::raw(" "));
            }
            tags.push(Span::styled(
                format!("#{tag}"),
                Style::default().fg(app.theme.tag),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(tags)), tags_area);
    }
    draw_rule(frame, rule_area, app.theme);
}

fn compose_preview(
    document: PreviewDocument,
    show_line_numbers: bool,
    theme: TuiTheme,
    width: u16,
) -> Text<'static> {
    let mut lines = Vec::new();
    // With line numbers enabled the code gutter intentionally occupies the
    // pane's first cell. Prose sections do not have a gutter, so inset them by
    // one cell to align with the `P` in the Preview block title/front matter.
    let prose_inset = usize::from(show_line_numbers);
    if let Some(note) = document.note {
        lines.push(inset_preview_line(note_header(theme), prose_inset));
        lines.extend(
            note.lines
                .into_iter()
                .map(|line| inset_preview_line(line, prose_inset)),
        );
        lines.push(inset_preview_line(
            note_footer(theme, width.saturating_sub(prose_inset as u16)),
            prose_inset,
        ));
    }

    let number_width = document.fragment.lines.len().max(1).to_string().len();
    for (index, line) in document.fragment.lines.into_iter().enumerate() {
        if show_line_numbers {
            let mut spans = vec![
                Span::styled(
                    format!("{:>number_width$}", index + 1),
                    Style::default().fg(theme.muted),
                ),
                Span::styled("│ ", Style::default().fg(theme.rule)),
            ];
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        } else {
            lines.push(line);
        }
    }

    if let Some(readme) = document.readme {
        lines.push(Line::default());
        lines.push(inset_preview_line(
            content_section_rule("readme", theme),
            prose_inset,
        ));
        lines.extend(
            readme
                .lines
                .into_iter()
                .map(|line| inset_preview_line(line, prose_inset)),
        );
    }
    Text::from(lines)
}

fn inset_preview_line(mut line: Line<'static>, inset: usize) -> Line<'static> {
    if inset > 0 {
        line.spans.insert(0, Span::raw(" ".repeat(inset)));
    }
    line
}

fn note_header(theme: TuiTheme) -> Line<'static> {
    Line::from(Span::styled(
        "Note",
        Style::default()
            .fg(theme.accent_alt)
            .add_modifier(Modifier::BOLD),
    ))
}

fn note_footer(theme: TuiTheme, width: u16) -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(width as usize),
        Style::default().fg(theme.rule),
    ))
}

fn content_section_rule(label: &str, theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled("── ", Style::default().fg(theme.rule)),
        Span::styled(
            label.to_owned(),
            Style::default().fg(theme.muted).add_modifier(Modifier::DIM),
        ),
        Span::styled(" ──", Style::default().fg(theme.rule)),
    ])
}

struct WrappedPreview {
    text: Text<'static>,
    rows: Vec<SelectionRow>,
}

fn wrap_preview(text: Text<'static>, width: u16, show_line_numbers: bool) -> WrappedPreview {
    let mut lines = Vec::new();
    let mut rows = Vec::new();
    for line in text.lines {
        let plain = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let decorative = is_preview_decoration(&plain);
        let number_gutter = if !decorative && show_line_numbers {
            line_number_gutter(&plain)
        } else {
            0
        };
        let prose_gutter = u16::from(
            !decorative && show_line_numbers && number_gutter == 0 && plain.starts_with(' '),
        );
        let line_gutter = if decorative {
            text_width(&plain)
        } else {
            number_gutter.max(prose_gutter)
        };
        let continuation = (number_gutter > 0)
            .then(|| {
                let padding = " ".repeat(number_gutter.saturating_sub(2) as usize);
                let number_style = line
                    .spans
                    .first()
                    .map_or(Style::default(), |span| span.style);
                let rule_style = line
                    .spans
                    .get(1)
                    .map_or(Style::default(), |span| span.style);
                (
                    vec![
                        Span::styled(padding.clone(), number_style),
                        Span::styled("│ ", rule_style),
                    ],
                    format!("{padding}│ "),
                )
            })
            .or_else(|| {
                (prose_gutter > 0).then(|| {
                    let style = line
                        .spans
                        .first()
                        .map_or(Style::default(), |span| span.style);
                    (vec![Span::styled(" ", style)], " ".to_owned())
                })
            });
        if line.spans.is_empty() {
            lines.push(Line::default());
            rows.push(SelectionRow {
                ends_line: true,
                ..SelectionRow::default()
            });
            continue;
        }

        let mut spans = Vec::new();
        let mut row_text = String::new();
        let mut row_width = 0_u16;
        let mut row_gutter = line_gutter;
        for span in line.spans {
            for character in span.content.chars() {
                let character_width = char_width(character);
                if row_width > 0 && row_width.saturating_add(character_width) > width {
                    push_preview_row(
                        &mut lines,
                        &mut rows,
                        std::mem::take(&mut spans),
                        std::mem::take(&mut row_text),
                        row_width,
                        row_gutter,
                        false,
                    );
                    if let Some((continuation_spans, continuation_text)) = &continuation {
                        spans = continuation_spans.clone();
                        row_text = continuation_text.clone();
                        row_width = line_gutter;
                        row_gutter = line_gutter;
                    } else {
                        row_width = 0;
                        row_gutter = 0;
                    }
                }
                row_width = row_width.saturating_add(character_width);
                row_text.push(character);
                spans.push(Span::styled(character.to_string(), span.style));
            }
        }
        push_preview_row(
            &mut lines,
            &mut rows,
            spans,
            row_text,
            row_width,
            row_gutter,
            !decorative,
        );
    }
    WrappedPreview {
        text: Text::from(lines),
        rows,
    }
}

fn is_preview_decoration(value: &str) -> bool {
    let value = value.trim_start();
    value == "Note"
        || value.starts_with("── readme ")
        || (!value.is_empty() && value.chars().all(|character| character == '─'))
}

fn push_preview_row(
    lines: &mut Vec<Line<'static>>,
    rows: &mut Vec<SelectionRow>,
    spans: Vec<Span<'static>>,
    text: String,
    width: u16,
    gutter_width: u16,
    ends_line: bool,
) {
    lines.push(Line::from(spans));
    rows.push(SelectionRow {
        text,
        display_width: width,
        gutter_width: gutter_width.min(width),
        ends_line,
    });
}

fn line_number_gutter(value: &str) -> u16 {
    let Some((number, remainder)) = value.split_once('│') else {
        return 0;
    };
    if number.trim().parse::<usize>().is_ok() && remainder.starts_with(' ') {
        text_width(number).saturating_add(text_width("│ "))
    } else {
        0
    }
}

fn draw_preview_selection(frame: &mut Frame<'_>, app: &App, area: Rect) {
    for screen_row in 0..area.height {
        let logical_row = app.preview_scroll as usize + screen_row as usize;
        for column in 0..area.width {
            if app.preview_selection.contains(logical_row, column) {
                let cell = &mut frame.buffer_mut()[(area.x + column, area.y + screen_row)];
                cell.fg = app.theme.selection_fg;
                cell.bg = app.theme.selection_bg;
            }
        }
    }
}

fn draw_rule(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(theme.rule)),
        area,
    );
}

fn truncate_end(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    value
        .chars()
        .take(width.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}

fn breadcrumb_spans(app: &App, width: usize, base: Style) -> Vec<Span<'static>> {
    let mut segments = if let Some(folder) = &app.filter.folder {
        folder.split('/').map(ToOwned::to_owned).collect::<Vec<_>>()
    } else if let Some(tag) = &app.filter.tag {
        vec![format!("#{tag}")]
    } else {
        vec!["All snippets".to_owned()]
    };
    let full_width = 1 + segments
        .iter()
        .map(|value| 3 + value.chars().count())
        .sum::<usize>();
    if full_width > width {
        let last = segments.pop().unwrap_or_default();
        if segments.is_empty() {
            segments = vec![truncate_end(&last, width.saturating_sub(4))];
        } else {
            segments = vec!["…".to_owned(), truncate_end(&last, width.saturating_sub(8))];
        }
    }
    let mut spans = vec![Span::styled("~", base.fg(app.theme.muted))];
    let last = segments.len().saturating_sub(1);
    for (index, segment) in segments.into_iter().enumerate() {
        spans.push(Span::styled(" › ", base.fg(app.theme.rule)));
        let style = if index == last {
            if segment.starts_with('#') {
                base.fg(app.theme.tag).add_modifier(Modifier::BOLD)
            } else if segment == "All snippets" {
                base.fg(app.theme.muted)
            } else {
                base.fg(app.theme.accent).add_modifier(Modifier::BOLD)
            }
        } else {
            base
        };
        spans.push(Span::styled(segment, style));
    }
    spans
}

fn draw_help(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    let popup = centered_rect(84, 30, area);
    frame.render_widget(Clear, popup);
    let help = Text::from(vec![
        Line::from(vec![
            Span::styled(
                "  snip TUI",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  ·  keyboard & mouse reference",
                Style::default().fg(theme.muted),
            ),
        ]),
        Line::default(),
        help_section("NAVIGATION", theme.accent, theme),
        help_row(
            ("Tab / Shift-Tab", "cycle panes"),
            ("h / ←   l / →", "back / drill in"),
            theme.accent,
            theme,
        ),
        help_row(
            ("j / k   ↑ / ↓", "move or scroll"),
            ("g / G", "top / bottom"),
            theme.accent,
            theme,
        ),
        help_row(
            ("Ctrl-d / Ctrl-u", "page down / up"),
            ("[ / ]", "fragment tab"),
            theme.accent,
            theme,
        ),
        Line::default(),
        help_section("SNIPPETS", theme.accent_alt, theme),
        help_row(
            ("n", "new snippet"),
            ("e / E / R", "content / note / README"),
            theme.accent_alt,
            theme,
        ),
        help_row(
            ("r / m / t", "rename / move / tags"),
            ("d", "move to trash"),
            theme.accent_alt,
            theme,
        ),
        help_row(
            ("p / L", "pin / lock"),
            ("y / Y", "copy content / UUID"),
            theme.accent_alt,
            theme,
        ),
        Line::default(),
        help_section("LIBRARY & GLOBAL", theme.tag, theme),
        help_row(
            ("n / r / d", "folder or tag actions"),
            ("/", "search"),
            theme.tag,
            theme,
        ),
        help_row(
            ("s / T", "sort / trash"),
            ("F5 / Ctrl-r", "rescan"),
            theme.tag,
            theme,
        ),
        help_row(
            ("Esc", "close or clear"),
            ("q / ?", "quit / help"),
            theme.tag,
            theme,
        ),
        Line::default(),
        help_section("PREVIEW & MOUSE", theme.success, theme),
        help_row(
            ("N", "toggle line numbers"),
            ("wheel", "scroll hovered pane"),
            theme.success,
            theme,
        ),
        help_row(
            ("click", "select item or tab"),
            ("double-click", "drill into preview"),
            theme.success,
            theme,
        ),
        help_row(
            ("drag in preview", "select text"),
            ("mouse up", "copy selection"),
            theme.success,
            theme,
        ),
        Line::default(),
        Line::from(vec![
            Span::styled(
                "  Esc",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" close help", Style::default().fg(theme.muted)),
        ]),
    ]);
    frame.render_widget(
        Paragraph::new(help)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(theme.accent)),
            )
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn help_section(label: &str, color: ratatui::style::Color, theme: TuiTheme) -> Line<'static> {
    Line::from(vec![
        Span::styled("  ── ", Style::default().fg(theme.rule)),
        Span::styled(
            label.to_owned(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ─────────────────────", Style::default().fg(theme.rule)),
    ])
}

fn help_row(
    left: (&str, &str),
    right: (&str, &str),
    key_color: ratatui::style::Color,
    theme: TuiTheme,
) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            pad_to(left.0, 17),
            Style::default().fg(key_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(pad_to(left.1, 22), Style::default().fg(theme.muted)),
        Span::styled(
            pad_to(right.0, 17),
            Style::default().fg(key_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(right.1.to_owned(), Style::default().fg(theme.muted)),
    ])
}

fn pad_to(value: &str, width: usize) -> String {
    let used = value.chars().count();
    format!("{value}{}", " ".repeat(width.saturating_sub(used)))
}

fn pane_block(title: &str, focused: bool, theme: TuiTheme) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            if focused {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.border }))
}

fn inset_left(area: Rect, amount: u16) -> Rect {
    let amount = amount.min(area.width);
    Rect {
        x: area.x.saturating_add(amount),
        width: area.width.saturating_sub(amount),
        ..area
    }
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
