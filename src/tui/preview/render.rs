use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::domain::Snippet;

use super::super::app::App;
use super::super::icons;
use super::super::selection::SelectionKey;
use super::super::state::Pane;
use super::super::widgets;
use super::layout::{compose_preview, wrap_preview};

pub fn draw_preview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let snippet = app.selected_snippet().cloned();
    let block = widgets::preview_block(
        app.focus == Pane::Preview,
        app.theme,
        snippet.as_ref(),
        app.fragment_index,
        area.width,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(snippet) = snippet else {
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
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner)
    } else {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner)
    };
    let title_area = regions[0];
    let metadata_area = regions[1];
    let fragment_area = regions[2];
    let tags_area = has_tags.then_some(regions[3]);
    let rule_area = if has_tags { regions[4] } else { regions[3] };
    let raw_content_area = if has_tags { regions[5] } else { regions[4] };
    let content_area = if app.show_line_numbers {
        raw_content_area
    } else {
        widgets::inset_left(raw_content_area, 1)
    };
    app.layout.preview_tabs = fragment_area;
    app.layout.preview_content = content_area;
    draw_preview_header(
        frame,
        app,
        &snippet,
        PreviewHeaderAreas {
            title: title_area,
            metadata: metadata_area,
            fragment: fragment_area,
            tags: tags_area,
            rule: rule_area,
        },
    );
    match app
        .preview
        .get(&snippet, app.fragment_index, &app.highlighter, app.theme)
    {
        Ok(document) => {
            let lines = compose_preview(
                document,
                app.show_line_numbers,
                app.theme,
                content_area.width.max(1),
            );
            let rendered = wrap_preview(lines, content_area.width.max(1), app.show_line_numbers);
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

struct PreviewHeaderAreas {
    title: Rect,
    metadata: Rect,
    fragment: Rect,
    tags: Option<Rect>,
    rule: Rect,
}

fn draw_preview_header(
    frame: &mut Frame<'_>,
    app: &mut App,
    snippet: &Snippet,
    areas: PreviewHeaderAreas,
) {
    let title_area = widgets::inset_left(areas.title, 1);
    let metadata_area = widgets::inset_left(areas.metadata, 1);
    let fragment_area = widgets::inset_left(areas.fragment, 1);
    let tags_area = areas.tags.map(|area| widgets::inset_left(area, 1));
    let rule_area = widgets::inset_left(areas.rule, 1);
    let marker = match (snippet.pinned, snippet.locked) {
        (true, true) => "★ pinned · ⊘ locked",
        (true, false) => "★ pinned",
        (false, true) => "⊘ locked",
        (false, false) => "",
    };
    let title_width = title_area
        .width
        .saturating_sub(marker.chars().count() as u16 + 2) as usize;
    let title = widgets::truncate_end(&snippet.title, title_width);
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
            Span::raw("   "),
        ])),
        title_area,
    );
    let metadata = vec![
        Span::styled(
            crate::domain::folder_label(&snippet.folder).to_owned(),
            Style::default().fg(app.theme.muted),
        ),
        Span::styled(" · ", Style::default().fg(app.theme.muted)),
        Span::styled(
            snippet.fingerprint.0.chars().take(8).collect::<String>(),
            Style::default().fg(app.theme.muted),
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(metadata)), metadata_area);

    let mut fragments_spans = Vec::new();
    let total_fragments = snippet.loaded_fragments.len();
    let current_fragment = app.fragment_index.saturating_add(1).min(total_fragments);

    if total_fragments > 1 {
        fragments_spans.push(Span::styled(
            "fragment ",
            Style::default().fg(app.theme.accent),
        ));
        fragments_spans.push(Span::styled(
            format!("{current_fragment}/{total_fragments}"),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        fragments_spans.push(Span::styled(
            " [",
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        fragments_spans.push(Span::styled(
            "]",
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ));
        fragments_spans.push(Span::styled(" · ", Style::default().fg(app.theme.rule)));
    } else {
        fragments_spans.push(Span::styled(
            "fragment ",
            Style::default().fg(app.theme.accent),
        ));
        fragments_spans.push(Span::styled(
            "1/1",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
        fragments_spans.push(Span::styled(" · ", Style::default().fg(app.theme.rule)));
    }

    let mut start = fragment_area
        .x
        .saturating_add(Line::from(fragments_spans.clone()).width() as u16);
    for (index, fragment) in snippet.loaded_fragments.iter().take(16).enumerate() {
        if index > 0 {
            let separator = " │ ";
            fragments_spans.push(Span::styled(separator, Style::default().fg(app.theme.rule)));
            start = start.saturating_add(separator.chars().count() as u16);
        }
        let file = std::path::Path::new(&fragment.file)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&fragment.title);
        let badge = icons::language_badge(&fragment.language);

        let full_text = if badge.is_empty() {
            file.to_owned()
        } else {
            format!("{file} {badge}")
        };

        let available = fragment_area.right().saturating_sub(start) as usize;
        let truncated = widgets::truncate_end(&full_text, available);
        let width = Line::raw(truncated.clone()).width() as u16;

        app.layout.tab_spans[index] = (start, start.saturating_add(width));
        app.layout.tab_count += 1;

        if badge.is_empty() || truncated.chars().count() < full_text.chars().count() {
            fragments_spans.push(Span::styled(
                truncated,
                if index == app.fragment_index {
                    Style::default().fg(app.theme.bar_fg)
                } else {
                    Style::default().fg(app.theme.muted)
                },
            ));
        } else {
            fragments_spans.push(Span::styled(
                file.to_owned(),
                if index == app.fragment_index {
                    Style::default().fg(app.theme.bar_fg)
                } else {
                    Style::default().fg(app.theme.muted)
                },
            ));
            fragments_spans.push(Span::raw(" "));
            fragments_spans.push(Span::styled(
                badge.to_owned(),
                Style::default().fg(app.theme.accent_alt),
            ));
        }

        start = start.saturating_add(width);
        if start >= fragment_area.right() {
            break;
        }
    }
    frame.render_widget(Paragraph::new(Line::from(fragments_spans)), fragment_area);

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
    widgets::draw_rule(frame, rule_area, app.theme);
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
