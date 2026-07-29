use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use super::super::app::App;
use super::super::selection::{SelectionRow, char_width, text_width};
use super::super::theme::TuiTheme;
use super::cache::PreviewDocument;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WrapMode {
    Character,
    Word,
}

pub(super) struct PreviewLine {
    line: Line<'static>,
    wrap: WrapMode,
}

pub(super) fn compose_preview(
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

pub(super) struct WrappedPreview {
    pub(super) text: Text<'static>,
    pub(super) rows: Vec<SelectionRow>,
}

pub(super) fn wrap_preview(
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
