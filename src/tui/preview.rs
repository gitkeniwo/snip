use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use crate::domain::Snippet;
use crate::error::Result;

use super::app::App;
use super::highlight::Highlighter;
use super::icons;
use super::selection::{SelectionKey, SelectionRow, char_width, text_width};
use super::theme::TuiTheme;
use super::widgets;

#[derive(Clone, Debug, Default)]
pub struct PreviewDocument {
    pub note: Option<Text<'static>>,
    pub fragment: Text<'static>,
    pub readme: Option<Text<'static>>,
}

#[derive(Default)]
pub struct PreviewCache {
    key: Option<(String, usize)>,
    document: Option<PreviewDocument>,
}

impl PreviewCache {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.document = None;
    }

    pub fn get(
        &mut self,
        snippet: &Snippet,
        fragment_index: usize,
        highlighter: &Highlighter,
        theme: TuiTheme,
    ) -> Result<PreviewDocument> {
        let key = (snippet.fingerprint.0.clone(), fragment_index);
        if self.key.as_ref() == Some(&key) {
            return Ok(self.document.clone().unwrap_or_default());
        }
        let document = build(snippet, fragment_index, highlighter, theme)?;
        self.key = Some(key);
        self.document = Some(document.clone());
        Ok(document)
    }
}

fn build(
    snippet: &Snippet,
    fragment_index: usize,
    highlighter: &Highlighter,
    theme: TuiTheme,
) -> Result<PreviewDocument> {
    let Some(fragment) = snippet.loaded_fragments.get(fragment_index) else {
        return Ok(PreviewDocument::default());
    };
    Ok(PreviewDocument {
        note: fragment
            .note_content
            .as_deref()
            .map(|note| highlighter.markdown(note, theme)),
        fragment: highlighter.fragment(fragment)?,
        readme: snippet
            .readme
            .as_deref()
            .map(|readme| highlighter.markdown(readme, theme)),
    })
}

pub fn draw_preview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let snippet = app.selected_snippet().cloned();
    let block = widgets::preview_block(
        app.focus == super::state::Pane::Preview,
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
pub fn jump_paragraph(app: &mut App, forward: bool) {
    let Some(snippet) = app.selected_snippet().cloned() else {
        return;
    };
    let width = app.layout.preview_content.width.max(1);
    let Ok(document) = app
        .preview
        .get(&snippet, app.fragment_index, &app.highlighter, app.theme)
    else {
        return;
    };
    let lines = compose_preview(document, app.show_line_numbers, app.theme, width);
    let rendered = wrap_preview(lines, width, app.show_line_numbers);
    let total_lines = rendered.text.lines.len();
    if total_lines == 0 {
        return;
    }

    let is_blank = |line: &ratatui::text::Line| {
        let plain = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let stripped = if let Some(pos) = plain.find('│') {
            &plain[pos + '│'.len_utf8()..]
        } else {
            &plain
        };
        stripped.trim().is_empty()
    };

    let current = usize::from(app.preview_scroll);
    let target = if forward {
        let mut i = current.saturating_add(1);
        if i >= total_lines {
            total_lines.saturating_sub(1)
        } else {
            if is_blank(&rendered.text.lines[current]) {
                while i < total_lines && is_blank(&rendered.text.lines[i]) {
                    i += 1;
                }
            }
            while i < total_lines && !is_blank(&rendered.text.lines[i]) {
                i += 1;
            }
            i.min(total_lines.saturating_sub(1))
        }
    } else if current == 0 {
        0
    } else {
        let mut i = current.saturating_sub(1);
        if is_blank(&rendered.text.lines[current]) {
            while i > 0 && is_blank(&rendered.text.lines[i]) {
                i -= 1;
            }
        }
        while i > 0 && !is_blank(&rendered.text.lines[i]) {
            i -= 1;
        }
        i
    };

    app.preview_scroll = u16::try_from(target).unwrap_or(u16::MAX);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WrapMode {
    Character,
    Word,
}

struct PreviewLine {
    line: Line<'static>,
    wrap: WrapMode,
}

fn compose_preview(
    document: PreviewDocument,
    show_line_numbers: bool,
    theme: TuiTheme,
    width: u16,
) -> Vec<PreviewLine> {
    let mut lines = Vec::new();
    let prose_inset = usize::from(show_line_numbers);
    if let Some(note) = document.note {
        lines.push(PreviewLine {
            line: inset_preview_line(note_header(theme), prose_inset),
            wrap: WrapMode::Character,
        });
        lines.extend(note.lines.into_iter().map(|line| PreviewLine {
            line: inset_preview_line(line, prose_inset),
            wrap: WrapMode::Word,
        }));
        lines.push(PreviewLine {
            line: inset_preview_line(
                note_footer(theme, width.saturating_sub(prose_inset as u16)),
                prose_inset,
            ),
            wrap: WrapMode::Character,
        });
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
            lines.push(PreviewLine {
                line: Line::from(spans),
                wrap: WrapMode::Character,
            });
        } else {
            lines.push(PreviewLine {
                line,
                wrap: WrapMode::Character,
            });
        }
    }

    if let Some(readme) = document.readme {
        lines.push(PreviewLine {
            line: Line::default(),
            wrap: WrapMode::Character,
        });
        lines.push(PreviewLine {
            line: inset_preview_line(content_section_rule("readme", theme), prose_inset),
            wrap: WrapMode::Character,
        });
        lines.extend(readme.lines.into_iter().map(|line| PreviewLine {
            line: inset_preview_line(line, prose_inset),
            wrap: WrapMode::Word,
        }));
    }
    lines
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

fn wrap_preview(
    preview_lines: Vec<PreviewLine>,
    width: u16,
    show_line_numbers: bool,
) -> WrappedPreview {
    let mut lines = Vec::new();
    let mut rows = Vec::new();
    for PreviewLine { line, wrap } in preview_lines {
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
        let mut continuation_chars = 0;
        for span in line.spans {
            for character in span.content.chars() {
                let character_width = char_width(character);
                if row_width > 0
                    && row_width.saturating_add(character_width) > width
                    && wrap == WrapMode::Word
                    && let Some(break_at) = spans
                        .iter()
                        .enumerate()
                        .skip(continuation_chars)
                        .rev()
                        .find_map(|(index, span): (usize, &Span<'static>)| {
                            span.content
                                .chars()
                                .next()
                                .is_some_and(char::is_whitespace)
                                .then_some(index + 1)
                        })
                    && break_at < spans.len()
                {
                    let tail = spans.split_off(break_at);
                    let head_text = spans_text(&spans);
                    let head_width = spans_width(&spans);
                    push_preview_row(
                        &mut lines,
                        &mut rows,
                        std::mem::take(&mut spans),
                        head_text,
                        head_width,
                        row_gutter,
                        false,
                    );
                    let (mut next_spans, next_text) = continuation.as_ref().map_or_else(
                        || (Vec::new(), String::new()),
                        |(prefix, text)| (expand_spans(prefix), text.clone()),
                    );
                    continuation_chars = next_spans.len();
                    next_spans.extend(tail);
                    spans = next_spans;
                    row_text =
                        format!("{next_text}{}", spans_text_from(&spans, continuation_chars));
                    row_width = spans_width(&spans);
                    row_gutter = line_gutter;
                }
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
                        spans = expand_spans(continuation_spans);
                        continuation_chars = spans.len();
                        row_text = continuation_text.clone();
                        row_width = line_gutter;
                        row_gutter = line_gutter;
                    } else {
                        continuation_chars = 0;
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

fn expand_spans(spans: &[Span<'static>]) -> Vec<Span<'static>> {
    spans
        .iter()
        .flat_map(|span| {
            span.content
                .chars()
                .map(|character| Span::styled(character.to_string(), span.style))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn spans_text(spans: &[Span<'static>]) -> String {
    spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn spans_text_from(spans: &[Span<'static>], start: usize) -> String {
    spans_text(&spans[start.min(spans.len())..])
}

fn spans_width(spans: &[Span<'static>]) -> u16 {
    spans
        .iter()
        .map(|span| text_width(span.content.as_ref()))
        .fold(0, u16::saturating_add)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered_lines(preview: &WrappedPreview) -> Vec<String> {
        preview
            .text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn prose_wraps_at_words_while_code_keeps_character_columns() {
        let prose = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("understanding complex topics"),
                wrap: WrapMode::Word,
            }],
            15,
            false,
        );
        assert_eq!(rendered_lines(&prose), ["understanding ", "complex topics"]);
        assert_eq!(prose.text.lines.len(), prose.rows.len());
        assert!(!prose.rows[0].ends_line);
        assert!(prose.rows[1].ends_line);
        assert_eq!(
            prose
                .rows
                .iter()
                .map(|row| row.text.as_str())
                .collect::<String>(),
            "understanding complex topics"
        );

        let code = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("averylongidentifier"),
                wrap: WrapMode::Character,
            }],
            8,
            false,
        );
        assert_eq!(rendered_lines(&code), ["averylon", "gidentif", "ier"]);
    }

    #[test]
    fn cjk_prose_falls_back_to_valid_character_boundaries() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("中文内容没有空格"),
                wrap: WrapMode::Word,
            }],
            6,
            false,
        );
        assert_eq!(rendered_lines(&preview), ["中文内", "容没有", "空格"]);
        assert_eq!(preview.text.lines.len(), preview.rows.len());
    }
}
