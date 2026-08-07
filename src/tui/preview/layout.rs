use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use unicode_segmentation::UnicodeSegmentation;

use super::super::app::App;
use super::super::selection::{SelectionRow, cluster_width, text_width};
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
    let prose_inset = usize::from(show_line_numbers);
    let (note, fragment) = match document {
        PreviewDocument::Fragment { note, body } => (note, body),
        // Just the file, exactly like a gist. The tree row is the label, so a
        // second one inside the pane would only repeat it.
        PreviewDocument::Readme(readme) => {
            return readme
                .lines
                .into_iter()
                .map(|line| PreviewLine {
                    line: inset_preview_line(line, prose_inset),
                    wrap: WrapMode::Word,
                })
                .collect();
        }
        PreviewDocument::Empty => return Vec::new(),
    };
    let mut lines = Vec::new();
    if let Some(note) = note {
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

    let number_width = fragment.lines.len().max(1).to_string().len();
    for (index, line) in fragment.lines.into_iter().enumerate() {
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

pub struct WrappedPreview {
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
        let mut continuation_bytes = 0;
        for span in line.spans {
            for cluster in span.content.graphemes(true) {
                let cluster_width = cluster_width(cluster);
                if row_width > 0
                    && row_width.saturating_add(cluster_width) > width
                    && wrap == WrapMode::Word
                    && let Some(break_at) = row_text
                        .grapheme_indices(true)
                        .filter(|(index, _)| *index >= continuation_bytes)
                        .rev()
                        .find_map(|(index, cluster)| {
                            cluster
                                .chars()
                                .all(char::is_whitespace)
                                .then_some(index + cluster.len())
                        })
                    && break_at < row_text.len()
                {
                    let (head, tail) = split_spans_at(std::mem::take(&mut spans), break_at);
                    let head_text = spans_text(&head);
                    let head_width = spans_width(&head);
                    push_preview_row(
                        &mut lines, &mut rows, head, head_text, head_width, row_gutter, false,
                    );
                    let (mut next_spans, next_text) = continuation.as_ref().map_or_else(
                        || (Vec::new(), String::new()),
                        |(prefix, text)| (prefix.clone(), text.clone()),
                    );
                    continuation_bytes = next_text.len();
                    next_spans.extend(tail);
                    spans = next_spans;
                    row_text = format!("{next_text}{}", &row_text[break_at..]);
                    row_width = spans_width(&spans);
                    row_gutter = line_gutter;
                }
                if row_width > 0 && row_width.saturating_add(cluster_width) > width {
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
                        continuation_bytes = continuation_text.len();
                        row_text = continuation_text.clone();
                        row_width = line_gutter;
                        row_gutter = line_gutter;
                    } else {
                        continuation_bytes = 0;
                        row_width = 0;
                        row_gutter = 0;
                    }
                }
                row_width = row_width.saturating_add(cluster_width);
                row_text.push_str(cluster);
                push_styled_cluster(&mut spans, cluster, span.style);
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

fn push_styled_cluster(spans: &mut Vec<Span<'static>>, cluster: &str, style: Style) {
    if let Some(last) = spans.last_mut()
        && last.style == style
    {
        last.content.to_mut().push_str(cluster);
    } else {
        spans.push(Span::styled(cluster.to_owned(), style));
    }
}

fn split_spans_at(
    spans: Vec<Span<'static>>,
    byte_offset: usize,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut remaining = byte_offset;
    for span in spans {
        let content_len = span.content.len();
        if remaining >= content_len {
            remaining -= content_len;
            head.push(span);
        } else if remaining == 0 {
            tail.push(span);
        } else {
            let content = span.content.into_owned();
            debug_assert!(
                content
                    .grapheme_indices(true)
                    .any(|(index, _)| index == remaining),
                "span split must fall on a grapheme boundary"
            );
            let (before, after) = content.split_at(remaining);
            head.push(Span::styled(before.to_owned(), span.style));
            tail.push(Span::styled(after.to_owned(), span.style));
            remaining = 0;
        }
    }
    (head, tail)
}

fn spans_text(spans: &[Span<'static>]) -> String {
    spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>()
}

fn spans_width(spans: &[Span<'static>]) -> u16 {
    spans
        .iter()
        .map(|span| text_width(span.content.as_ref()))
        .fold(0, u16::saturating_add)
}

fn is_preview_decoration(value: &str) -> bool {
    let value = value.trim_start();
    value == "Note" || (!value.is_empty() && value.chars().all(|character| character == '─'))
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
    let mut preview_cache = std::mem::take(&mut app.preview);
    let target = (|| {
        let (rendered, rebuilt) = preview_cache
            .get(
                &snippet,
                app.preview_target,
                width,
                app.show_line_numbers,
                &app.highlighter,
                app.theme,
            )
            .ok()?;
        let selection_key = super::super::selection::SelectionKey {
            snippet_id: snippet.id,
            target: app.preview_target,
            fingerprint: snippet.fingerprint.0.clone(),
        };
        if rebuilt || !app.preview_selection.is_prepared_for(&selection_key) {
            app.preview_selection
                .prepare(selection_key, rendered.rows.clone());
        } else {
            app.preview_selection.reclamp();
        }
        let total_lines = rendered.text.lines.len();
        if total_lines == 0 {
            return None;
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
        Some(if forward {
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
        })
    })();
    app.preview = preview_cache;
    if let Some(target) = target {
        app.preview_scroll = u16::try_from(target).unwrap_or(u16::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Eq, PartialEq)]
    struct ExpectedRow<'a> {
        text: &'a str,
        display_width: u16,
        gutter_width: u16,
        ends_line: bool,
    }

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

    fn assert_rows(preview: &WrappedPreview, expected: &[ExpectedRow<'_>]) {
        assert_eq!(preview.text.lines.len(), preview.rows.len());
        assert_eq!(preview.rows.len(), expected.len());
        for (actual, expected) in preview.rows.iter().zip(expected) {
            assert_eq!(actual.text, expected.text);
            assert_eq!(actual.display_width, expected.display_width);
            assert_eq!(actual.gutter_width, expected.gutter_width);
            assert_eq!(actual.ends_line, expected.ends_line);
        }
    }

    fn row(
        text: &'static str,
        display_width: u16,
        gutter_width: u16,
        ends_line: bool,
    ) -> ExpectedRow<'static> {
        ExpectedRow {
            text,
            display_width,
            gutter_width,
            ends_line,
        }
    }

    fn style_at_column(line: &Line<'_>, column: u16) -> Style {
        let mut current = 0;
        for span in &line.spans {
            for cluster in span.content.graphemes(true) {
                let next = current + cluster_width(cluster);
                if column < next {
                    return span.style;
                }
                current = next;
            }
        }
        Style::default()
    }

    #[test]
    fn code_wraps_at_character_columns_and_preserves_row_metadata() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("abcdefghij"),
                wrap: WrapMode::Character,
            }],
            4,
            false,
        );

        assert_eq!(rendered_lines(&preview), ["abcd", "efgh", "ij"]);
        assert_rows(
            &preview,
            &[
                row("abcd", 4, 0, false),
                row("efgh", 4, 0, false),
                row("ij", 2, 0, true),
            ],
        );
    }

    #[test]
    fn prose_wraps_at_whitespace_and_preserves_row_metadata() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("alpha beta gamma"),
                wrap: WrapMode::Word,
            }],
            10,
            false,
        );

        assert_eq!(rendered_lines(&preview), ["alpha ", "beta gamma"]);
        assert_rows(
            &preview,
            &[row("alpha ", 6, 0, false), row("beta gamma", 10, 0, true)],
        );
    }

    #[test]
    fn line_number_continuations_repeat_the_rule_gutter() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::from(vec![
                    Span::styled("12", Style::default().fg(ratatui::style::Color::Red)),
                    Span::styled("│ ", Style::default().fg(ratatui::style::Color::Blue)),
                    Span::raw("abcdef"),
                ]),
                wrap: WrapMode::Character,
            }],
            6,
            true,
        );

        assert_eq!(rendered_lines(&preview), ["12│ ab", "  │ cd", "  │ ef"]);
        assert_rows(
            &preview,
            &[
                row("12│ ab", 6, 4, false),
                row("  │ cd", 6, 4, false),
                row("  │ ef", 6, 4, true),
            ],
        );
        assert_eq!(
            style_at_column(&preview.text.lines[1], 0).fg,
            Some(ratatui::style::Color::Red)
        );
        assert_eq!(
            style_at_column(&preview.text.lines[1], 2).fg,
            Some(ratatui::style::Color::Blue)
        );
    }

    #[test]
    fn prose_inset_is_repeated_on_continuation_rows() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw(" alpha beta gamma"),
                wrap: WrapMode::Word,
            }],
            8,
            true,
        );

        assert_eq!(rendered_lines(&preview), [" alpha ", " beta ", " gamma"]);
        assert_rows(
            &preview,
            &[
                row(" alpha ", 7, 1, false),
                row(" beta ", 6, 1, false),
                row(" gamma", 6, 1, true),
            ],
        );
    }

    /// The README used to be appended below the fragment under a `── readme ──`
    /// rule. It is its own target now, so the pane is just the file.
    #[test]
    fn readme_preview_has_no_section_rule() {
        let theme = TuiTheme::from(&crate::theme::load("dark-default").unwrap());
        let readme = Text::from(vec![Line::raw("alpha beta"), Line::raw("gamma")]);
        let preview = wrap_preview(
            compose_preview(PreviewDocument::Readme(readme), false, theme, 40),
            40,
            false,
        );

        assert_eq!(rendered_lines(&preview), ["alpha beta", "gamma"]);
        for line in rendered_lines(&preview) {
            assert!(!line.contains("──"), "readme line: {line:?}");
            assert!(!line.trim().is_empty(), "readme line: {line:?}");
        }
    }

    #[test]
    fn an_empty_document_renders_nothing() {
        let theme = TuiTheme::from(&crate::theme::load("dark-default").unwrap());
        assert!(compose_preview(PreviewDocument::Empty, false, theme, 40).is_empty());
    }

    #[test]
    fn decorations_do_not_acquire_line_or_prose_gutters() {
        for decoration in ["Note", "────"] {
            let preview = wrap_preview(
                vec![PreviewLine {
                    line: Line::raw(decoration),
                    wrap: WrapMode::Character,
                }],
                20,
                true,
            );

            assert_eq!(rendered_lines(&preview), [decoration]);
            assert_rows(
                &preview,
                &[row(
                    decoration,
                    text_width(decoration),
                    text_width(decoration),
                    false,
                )],
            );
        }
    }

    #[test]
    fn empty_line_has_a_terminal_default_selection_row() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::default(),
                wrap: WrapMode::Character,
            }],
            10,
            false,
        );

        assert_eq!(rendered_lines(&preview), [""]);
        assert_rows(&preview, &[row("", 0, 0, true)]);
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
        assert_rows(
            &preview,
            &[
                row("中文内", 6, 0, false),
                row("容没有", 6, 0, false),
                row("空格", 4, 0, true),
            ],
        );
    }

    #[test]
    fn prose_word_longer_than_width_falls_back_to_character_columns() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("supercalifragilistic"),
                wrap: WrapMode::Word,
            }],
            7,
            false,
        );

        assert_eq!(rendered_lines(&preview), ["superca", "lifragi", "listic"]);
        assert_rows(
            &preview,
            &[
                row("superca", 7, 0, false),
                row("lifragi", 7, 0, false),
                row("listic", 6, 0, true),
            ],
        );
    }

    #[test]
    fn span_count_tracks_style_runs_instead_of_character_count() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::styled(
                    "x".repeat(200),
                    Style::default().fg(ratatui::style::Color::Red),
                ),
                wrap: WrapMode::Character,
            }],
            200,
            false,
        );

        assert_eq!(preview.text.lines.len(), 1);
        assert!(preview.text.lines[0].spans.len() < 5);
    }

    #[test]
    fn zwj_emoji_wraps_at_word_boundaries_using_cluster_width() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("echo 👨\u{200d}💻 👨\u{200d}👩\u{200d}👧 café done"),
                wrap: WrapMode::Word,
            }],
            20,
            false,
        );

        assert_eq!(
            rendered_lines(&preview),
            ["echo 👨\u{200d}💻 👨\u{200d}👩\u{200d}👧 café done"]
        );
        assert_rows(
            &preview,
            &[row(
                "echo 👨\u{200d}💻 👨\u{200d}👩\u{200d}👧 café done",
                20,
                0,
                true,
            )],
        );
    }

    #[test]
    fn character_wrap_never_splits_a_zwj_emoji() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("👨\u{200d}💻x"),
                wrap: WrapMode::Character,
            }],
            2,
            false,
        );

        assert_eq!(rendered_lines(&preview), ["👨\u{200d}💻", "x"]);
        assert_rows(
            &preview,
            &[row("👨\u{200d}💻", 2, 0, false), row("x", 1, 0, true)],
        );
    }

    #[test]
    fn nfd_grapheme_uses_one_display_cell_when_wrapping() {
        let preview = wrap_preview(
            vec![PreviewLine {
                line: Line::raw("e\u{301}x"),
                wrap: WrapMode::Character,
            }],
            1,
            false,
        );

        assert_eq!(rendered_lines(&preview), ["e\u{301}", "x"]);
        assert_rows(
            &preview,
            &[row("e\u{301}", 1, 0, false), row("x", 1, 0, true)],
        );
    }
}
