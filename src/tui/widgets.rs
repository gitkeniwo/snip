use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use unicode_segmentation::UnicodeSegmentation;

use super::icons;
use super::preview::PreviewTarget;
use super::selection::{cluster_width, text_width};
use super::theme::TuiTheme;
use crate::domain::Snippet;

const PILL_OPEN: &str = "\u{e0b6}";
const PILL_CLOSE: &str = "\u{e0b4}";

pub fn fill_surface(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    if let Some(style) = theme.surface_style() {
        frame.render_widget(Block::default().style(style), area);
    }
}

pub fn pane_block(title: &str, focused: bool, theme: TuiTheme) -> Block<'static> {
    pane_block_tinted(title, focused, theme, theme.accent)
}

/// A pane block whose focused colour is overridden — the trash view uses this to
/// tint its list and preview panes without duplicating the block construction.
pub fn pane_block_tinted(
    title: &str,
    focused: bool,
    theme: TuiTheme,
    accent: ratatui::style::Color,
) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            if focused {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { accent } else { theme.border }))
}

pub fn preview_block(
    focused: bool,
    theme: TuiTheme,
    snippet: Option<&Snippet>,
    target: PreviewTarget,
    width: u16,
) -> Block<'static> {
    preview_block_tinted(focused, theme, theme.accent, snippet, target, width)
}

pub fn preview_block_tinted(
    focused: bool,
    theme: TuiTheme,
    accent: ratatui::style::Color,
    snippet: Option<&Snippet>,
    target: PreviewTarget,
    width: u16,
) -> Block<'static> {
    let mut block = pane_block_tinted("Preview", focused, theme, accent);
    let Some(fragment) = target
        .fragment_index()
        .and_then(|index| snippet.and_then(|snippet| snippet.loaded_fragments.get(index)))
    else {
        return block;
    };

    let language = icons::language_name(&fragment.language);
    let line_count = fragment.content.lines().count();
    let count = format!(
        "{line_count} line{}",
        if line_count == 1 { "" } else { "s" }
    );
    block = block.title_bottom(
        Line::from(Span::styled(
            format!(" {language} "),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ))
        .left_aligned(),
    );

    let available = width.saturating_sub(2) as usize;
    if language.chars().count() + count.chars().count() + 4 < available {
        block = block.title_bottom(
            Line::from(vec![
                Span::styled(
                    format!(" {line_count}"),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" line{} ", if line_count == 1 { "" } else { "s" }),
                    Style::default().fg(theme.muted),
                ),
            ])
            .right_aligned(),
        );
    }
    block
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}

pub fn inset_left(area: Rect, amount: u16) -> Rect {
    let amount = amount.min(area.width);
    Rect {
        x: area.x.saturating_add(amount),
        width: area.width.saturating_sub(amount),
        ..area
    }
}

pub fn draw_rule(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(theme.rule)),
        area,
    );
}

/// Truncates `value` to `width` display cells, keeping a trailing ellipsis.
/// Cell-aware rather than character-aware: a wide glyph (CJK, a full-width
/// star) occupies two cells, and counting it as one lets the caller's padding
/// or flush-right spans overrun the pane and get clipped at the right edge.
pub fn truncate_end(value: &str, width: usize) -> String {
    if text_width(value) as usize <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let target = width.saturating_sub(1);
    let mut out = String::new();
    let mut cells = 0;
    for cluster in value.graphemes(true) {
        let cluster_width = cluster_width(cluster) as usize;
        if cells + cluster_width > target {
            break;
        }
        out.push_str(cluster);
        cells += cluster_width;
    }
    out.push('…');
    out
}

#[derive(Clone, Copy)]
pub struct PillCaps {
    simplified: bool,
}

impl PillCaps {
    pub const fn new(simplified: bool) -> Self {
        Self { simplified }
    }

    pub fn open(self, fill: Color, surround: Color) -> Span<'static> {
        self.cap(PILL_OPEN, fill, surround)
    }

    pub fn close(self, fill: Color, surround: Color) -> Span<'static> {
        self.cap(PILL_CLOSE, fill, surround)
    }

    fn cap(self, symbol: &'static str, fill: Color, surround: Color) -> Span<'static> {
        if self.simplified {
            // Preserve the Powerline cap's one-cell layout budget while replacing
            // its private-use glyph with a square extension of the pill fill.
            // square_start/square_end intentionally leave this identical span alone.
            return Span::styled(" ", Style::default().bg(fill));
        }
        let style = if fill == Color::Reset {
            // A powerline cap uses its foreground as the visible pill fill. ANSI's
            // default foreground is the terminal text colour, not its canvas, so a
            // terminal-inherited fill must swap the default background into the
            // glyph with reverse video.
            Style::default()
                .fg(surround)
                .bg(fill)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(fill).bg(surround)
        };
        Span::styled(symbol, style)
    }
}

fn pill_cap_fill(span: &Span<'_>) -> Option<ratatui::style::Color> {
    if span.style.add_modifier.contains(Modifier::REVERSED) {
        span.style.bg
    } else {
        span.style.fg
    }
}

/// Replace a leading pill cap with a flush square edge in the pill's own fill.
pub fn square_start(mut line: Line<'static>) -> Line<'static> {
    if let Some(span) = line.spans.first_mut()
        && span.content == PILL_OPEN
        && let Some(fill) = pill_cap_fill(span)
    {
        *span = Span::styled(" ", Style::default().bg(fill));
    }
    line
}

/// Replace a trailing pill cap with a flush square edge in the pill's own fill.
pub fn square_end(mut line: Line<'static>) -> Line<'static> {
    if let Some(span) = line.spans.last_mut()
        && span.content == PILL_CLOSE
        && let Some(fill) = pill_cap_fill(span)
    {
        *span = Span::styled(" ", Style::default().bg(fill));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn powerline_caps_keep_the_outer_pill_direction_and_colors() {
        let caps = PillCaps::new(false);
        let open = caps.open(Color::Cyan, Color::Black);
        let close = caps.close(Color::Cyan, Color::Black);
        assert_eq!(open.content, PILL_OPEN);
        assert_eq!(close.content, PILL_CLOSE);
        assert_eq!(open.style.fg, Some(Color::Cyan));
        assert_eq!(open.style.bg, Some(Color::Black));
        assert_eq!(close.style.fg, Some(Color::Cyan));
        assert_eq!(close.style.bg, Some(Color::Black));
    }

    #[test]
    fn terminal_fill_caps_reverse_the_default_background_into_the_glyph() {
        let cap = PillCaps::new(false).close(Color::Reset, Color::Blue);

        assert_eq!(cap.style.fg, Some(Color::Blue));
        assert_eq!(cap.style.bg, Some(Color::Reset));
        assert!(cap.style.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn square_edges_replace_caps_with_their_fill_without_changing_width() {
        let caps = PillCaps::new(false);
        let line = Line::from(vec![
            caps.open(Color::Cyan, Color::Black),
            Span::styled("pill", Style::default().bg(Color::Cyan)),
            caps.close(Color::Cyan, Color::Black),
        ]);
        let width = line.width();

        let line = square_end(square_start(line));

        assert_eq!(line.width(), width);
        assert_eq!(line.spans.first().unwrap().content, " ");
        assert_eq!(line.spans.first().unwrap().style.bg, Some(Color::Cyan));
        assert_eq!(line.spans.last().unwrap().content, " ");
        assert_eq!(line.spans.last().unwrap().style.bg, Some(Color::Cyan));
    }

    #[test]
    fn square_edges_leave_empty_and_uncapped_lines_unchanged() {
        let empty = Line::default();
        assert_eq!(square_start(empty.clone()), empty);
        assert_eq!(square_end(empty.clone()), empty);

        let plain = Line::from("plain");
        assert_eq!(square_start(plain.clone()), plain);
        assert_eq!(square_end(plain.clone()), plain);
    }

    #[test]
    fn square_edges_preserve_a_terminal_inherited_fill() {
        let caps = PillCaps::new(false);
        let line = Line::from(vec![
            caps.open(Color::Reset, Color::Blue),
            caps.close(Color::Reset, Color::Blue),
        ]);

        let line = square_end(square_start(line));

        assert_eq!(line.spans.first().unwrap().style.bg, Some(Color::Reset));
        assert_eq!(line.spans.last().unwrap().style.bg, Some(Color::Reset));
        assert!(
            !line
                .spans
                .first()
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::REVERSED)
        );
        assert!(
            !line
                .spans
                .last()
                .unwrap()
                .style
                .add_modifier
                .contains(Modifier::REVERSED)
        );
    }

    #[test]
    fn simplified_caps_are_square_fill_cells_with_the_same_width() {
        let powerline = PillCaps::new(false);
        let simplified = PillCaps::new(true);
        for (powerline, simplified, fill) in [
            (
                powerline.open(Color::Cyan, Color::Blue),
                simplified.open(Color::Cyan, Color::Blue),
                Color::Cyan,
            ),
            (
                powerline.close(Color::Reset, Color::Blue),
                simplified.close(Color::Reset, Color::Blue),
                Color::Reset,
            ),
        ] {
            assert_eq!(simplified.content, " ");
            assert_eq!(simplified.width(), powerline.width());
            assert_eq!(simplified.style.bg, Some(fill));
            assert_eq!(simplified.style.fg, None);
            assert!(!simplified.style.add_modifier.contains(Modifier::REVERSED));
        }
    }

    #[test]
    fn square_edges_are_no_ops_for_simplified_caps() {
        let caps = PillCaps::new(true);
        let line = Line::from(vec![
            caps.open(Color::Cyan, Color::Black),
            Span::styled("pill", Style::default().bg(Color::Cyan)),
            caps.close(Color::Cyan, Color::Black),
        ]);

        assert_eq!(square_end(square_start(line.clone())), line);
    }

    #[test]
    fn truncation_preserves_unicode_characters_and_adds_an_ellipsis() {
        assert_eq!(truncate_end("short", 8), "short");
        assert_eq!(truncate_end("abcdef", 3), "ab…");
    }

    #[test]
    fn truncation_measures_wide_glyphs_in_cells_not_characters() {
        // "你好" is four cells wide, so it only fits once the budget reaches 4.
        assert_eq!(truncate_end("你好 Rust", 5), "你好…");
        assert_eq!(truncate_end("代码评审", 4), "代…");
        assert_eq!(truncate_end("代码评审", 5), "代码…");
        assert_eq!(truncate_end("代码评审", 8), "代码评审");
        assert_eq!(truncate_end("代码评审", 0), "");
        // A budget of one cell cannot hold a wide glyph — only the ellipsis.
        assert_eq!(truncate_end("代码", 1), "…");
    }

    #[test]
    fn truncation_never_splits_a_zwj_grapheme_cluster() {
        let value = "👨\u{200d}💻 title";
        let boundaries = value
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(value.len()))
            .collect::<Vec<_>>();

        for width in 0..=8 {
            let truncated = truncate_end(value, width);
            let prefix = truncated.strip_suffix('…').unwrap_or(&truncated);
            assert!(value.starts_with(prefix), "width {width}: {truncated:?}");
            assert!(
                boundaries.contains(&prefix.len()),
                "width {width}: {truncated:?}"
            );
            assert!(
                !prefix.ends_with('\u{200d}'),
                "width {width}: {truncated:?}"
            );
        }

        assert_eq!(truncate_end(value, 4), "👨\u{200d}💻 …");
    }
}
