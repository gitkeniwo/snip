use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::app::App;
use super::command::{self, CommandId, CommandState};
use super::modal::TextInput;
use super::selection::text_width;
use super::widgets;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteMatch {
    pub id: CommandId,
    pub score: u32,
    pub indices: Vec<u32>,
}

pub struct PaletteState {
    pub open: bool,
    pub input: TextInput,
    pub matches: Vec<PaletteMatch>,
    pub selected: usize,
    pub scroll: usize,
    pub visible_rows: usize,
    recent: Vec<CommandId>,
    matcher: Matcher,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            open: false,
            input: TextInput::default(),
            matches: Vec::new(),
            selected: 0,
            scroll: 0,
            visible_rows: 10,
            recent: Vec::new(),
            matcher: Matcher::new(Config::DEFAULT),
        }
    }
}

impl PaletteState {
    pub fn open(&mut self) {
        self.open = true;
        self.input = TextInput::default();
        self.selected = 0;
        self.scroll = 0;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn record_recent(&mut self, id: CommandId) {
        self.recent.retain(|recent| *recent != id);
        self.recent.insert(0, id);
        self.recent.truncate(MAX_RECENT);
    }
    pub fn set_recent(&mut self, recent: Vec<CommandId>) {
        self.recent.clear();
        for id in recent {
            if !self.recent.contains(&id) {
                self.recent.push(id);
            }
        }
        self.recent.truncate(MAX_RECENT);
    }
    pub fn refresh(&mut self, hidden: &std::collections::HashSet<CommandId>) {
        let query = self.input.value.trim();
        if query.is_empty() {
            let mut ids = self
                .recent
                .iter()
                .copied()
                .filter(|id| !hidden.contains(id))
                .collect::<Vec<_>>();
            let remaining = command::registry()
                .iter()
                .map(|command| command.id)
                .filter(|id| !hidden.contains(id) && !ids.contains(id))
                .collect::<Vec<_>>();
            ids.extend(remaining);
            self.matches = ids
                .into_iter()
                .enumerate()
                .map(|(index, id)| PaletteMatch {
                    id,
                    score: u32::MAX.saturating_sub(index as u32),
                    indices: Vec::new(),
                })
                .collect();
        } else {
            let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
            let mut matches = command::registry()
                .iter()
                .enumerate()
                .filter(|(_, command)| !hidden.contains(&command.id))
                .filter_map(|(index, command)| {
                    let haystack = format!("{}: {}", command.category, command.title);
                    let mut indices = Vec::new();
                    let mut haystack_buf = Vec::new();
                    let mut score = pattern.indices(
                        Utf32Str::new(&haystack, &mut haystack_buf),
                        &mut self.matcher,
                        &mut indices,
                    )?;
                    for keyword in command.keywords {
                        let mut keyword_buf = Vec::new();
                        if let Some(keyword_score) = pattern
                            .score(Utf32Str::new(keyword, &mut keyword_buf), &mut self.matcher)
                        {
                            score = score.saturating_add(keyword_score);
                        }
                    }
                    Some((
                        score,
                        index,
                        PaletteMatch {
                            id: command.id,
                            score,
                            indices,
                        },
                    ))
                })
                .collect::<Vec<_>>();
            matches.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
            self.matches = matches.into_iter().map(|(_, _, matched)| matched).collect();
        }
        self.selected = 0;
        self.scroll = 0;
    }
    pub fn move_selection(&mut self, delta: isize) {
        if self.matches.is_empty() {
            return;
        }
        self.selected =
            (self.selected as isize + delta).clamp(0, self.matches.len() as isize - 1) as usize;
        self.ensure_selected_visible();
    }

    fn ensure_selected_visible(&mut self) {
        if self.matches.is_empty() {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = self.selected.min(self.matches.len() - 1);
        let visible_rows = self.visible_rows.max(1);
        self.scroll = self
            .scroll
            .min(self.matches.len().saturating_sub(visible_rows));
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + visible_rows {
            self.scroll = self.selected + 1 - visible_rows;
        }
    }
    pub fn selected_id(&self) -> Option<CommandId> {
        self.matches.get(self.selected).map(|matched| matched.id)
    }
    pub fn recent(&self) -> &[CommandId] {
        &self.recent
    }
}

pub const MAX_RECENT: usize = 20;

pub fn draw_palette(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    if !app.palette.open {
        return;
    }
    let overlay = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: if area.height <= 5 {
            area.height
        } else {
            area.height.saturating_sub(1)
        },
    };
    let width = overlay.width.saturating_mul(4).saturating_div(5).min(72);
    let rows = if app.palette.matches.is_empty() {
        1
    } else {
        app.palette.matches.len().min(10)
    } as u16;
    let height = (2 + 1 + 1 + rows).min(overlay.height);
    let mut popup = super::widgets::centered_rect(width, height, overlay);
    popup.y = area
        .y
        .saturating_add(area.height / 6)
        .min(overlay.bottom().saturating_sub(height));
    frame.render_widget(Clear, popup);
    app.palette.visible_rows = (popup.height.saturating_sub(4) as usize).max(1);
    app.palette.ensure_selected_visible();
    let mut block = Block::default()
        .title(" Commands ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent));
    if app.palette.matches.len() > app.palette.visible_rows {
        block = block.title_bottom(
            Line::from(format!(
                " {}/{} ",
                app.palette.selected + 1,
                app.palette.matches.len()
            ))
            .right_aligned(),
        );
    }
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let cursor = app
        .palette
        .input
        .cursor
        .min(app.palette.input.value.chars().count());
    let mut prompt = vec![Span::styled(
        ":",
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD),
    )];
    for (index, character) in app.palette.input.value.chars().enumerate() {
        if index == cursor {
            prompt.push(Span::styled(" ", Style::default().bg(app.theme.accent)));
        }
        prompt.push(Span::raw(character.to_string()));
    }
    if cursor == app.palette.input.value.chars().count() {
        prompt.push(Span::styled(" ", Style::default().bg(app.theme.accent)));
    }
    let prompt = Line::from(prompt);
    frame.render_widget(
        Paragraph::new(prompt),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );
    frame.render_widget(
        Paragraph::new("─".repeat(inner.width as usize))
            .style(Style::default().fg(app.theme.border)),
        Rect {
            x: inner.x,
            y: inner.y.saturating_add(1),
            width: inner.width,
            height: 1,
        },
    );
    if app.palette.matches.is_empty() {
        frame.render_widget(
            Paragraph::new("no matching command").style(Style::default().fg(app.theme.muted)),
            Rect {
                x: inner.x,
                y: inner.y.saturating_add(2),
                width: inner.width,
                height: 1,
            },
        );
        return;
    }
    for (row, matched) in app
        .palette
        .matches
        .iter()
        .skip(app.palette.scroll)
        .take(app.palette.visible_rows)
        .enumerate()
    {
        let command = command::get(matched.id);
        let full_text = format!("{}: {}", command.category, command.title);
        let state = (command.state)(app);
        let (hint, disabled) = match state {
            CommandState::Enabled => (command.key_hint.unwrap_or(""), false),
            CommandState::Disabled(reason) => (reason, true),
            CommandState::Hidden => continue,
        };
        let hint = widgets::truncate_end(hint, (inner.width / 2) as usize);
        let text_limit = inner
            .width
            .saturating_sub(text_width(&hint))
            .saturating_sub(1) as usize;
        let text = widgets::truncate_end(&full_text, text_limit);
        let selected = app.palette.selected == app.palette.scroll + row;
        let base = if disabled {
            Style::default().fg(app.theme.muted)
        } else if selected {
            app.theme.selected()
        } else {
            Style::default()
        };
        let mut spans = Vec::new();
        let index_set = matched
            .indices
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        for (index, character) in text.chars().enumerate() {
            spans.push(Span::styled(
                character.to_string(),
                if index_set.contains(&(index as u32)) && !disabled {
                    base.fg(app.theme.accent).add_modifier(Modifier::BOLD)
                } else {
                    base
                },
            ));
        }
        let padding = inner
            .width
            .saturating_sub(text_width(&text))
            .saturating_sub(text_width(&hint));
        spans.push(Span::styled(" ".repeat(padding as usize), base));
        spans.push(Span::styled(&hint, Style::default().fg(app.theme.muted)));
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: inner.x,
                y: inner.y.saturating_add(2 + row as u16),
                width: inner.width,
                height: 1,
            },
        );
    }
}
