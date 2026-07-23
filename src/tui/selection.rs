use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionKey {
    pub snippet_id: Uuid,
    pub fragment_index: usize,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Default)]
pub struct SelectionRow {
    pub text: String,
    pub display_width: u16,
    pub gutter_width: u16,
    pub ends_line: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SelectionPoint {
    pub row: usize,
    pub column: u16,
}

#[derive(Clone, Debug, Default)]
pub struct PreviewSelection {
    key: Option<SelectionKey>,
    rows: Vec<SelectionRow>,
    anchor: Option<SelectionPoint>,
    head: Option<SelectionPoint>,
    dragging: bool,
}

impl PreviewSelection {
    pub fn prepare(&mut self, key: SelectionKey, rows: Vec<SelectionRow>) {
        if self.key.as_ref() != Some(&key) {
            self.clear();
            self.key = Some(key);
        }
        self.rows = rows;
        self.clamp_points();
    }

    pub fn clear(&mut self) {
        self.anchor = None;
        self.head = None;
        self.dragging = false;
    }

    pub fn point_at(&self, row: usize, column: u16) -> Option<SelectionPoint> {
        let row_data = self.rows.get(row)?;
        let column = column.min(row_data.display_width.saturating_sub(1));
        Some(SelectionPoint { row, column })
    }

    pub fn begin(&mut self, point: SelectionPoint) {
        self.anchor = Some(point);
        self.head = Some(point);
        self.dragging = true;
    }

    pub fn update(&mut self, point: SelectionPoint) {
        if self.dragging {
            self.head = Some(point);
        }
    }

    pub fn finish(&mut self, point: SelectionPoint) -> Option<String> {
        if !self.dragging {
            return None;
        }
        if self.anchor == Some(point) {
            self.clear();
            return None;
        }
        self.head = Some(point);
        self.dragging = false;
        let text = self.selected_text();
        (!text.is_empty()).then_some(text)
    }

    pub fn contains(&self, row: usize, column: u16) -> bool {
        let Some((start, end)) = self.bounds() else {
            return false;
        };
        if row < start.row || row > end.row {
            return false;
        }
        let from = if row == start.row { start.column } else { 0 };
        let to = if row == end.row {
            end.column.saturating_add(1)
        } else {
            u16::MAX
        };
        self.rows
            .get(row)
            .is_some_and(|value| value.selects_column(column, from, to))
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    fn selected_text(&self) -> String {
        let Some((start, end)) = self.bounds() else {
            return String::new();
        };
        let mut output = String::new();
        for row_index in start.row..=end.row {
            let Some(row) = self.rows.get(row_index) else {
                continue;
            };
            let from = if row_index == start.row {
                start.column
            } else {
                0
            };
            let to = if row_index == end.row {
                end.column.saturating_add(1)
            } else {
                u16::MAX
            };
            output.push_str(&row.text_between(from, to));
            if row_index < end.row && row.ends_line {
                output.push('\n');
            }
        }
        output
    }

    fn bounds(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        let anchor = self.anchor?;
        let head = self.head?;
        Some(if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        })
    }

    fn clamp_points(&mut self) {
        let clamp = |point: SelectionPoint, rows: &[SelectionRow]| {
            let row = point.row.min(rows.len().saturating_sub(1));
            let column = rows.get(row).map_or(0, |value| {
                point.column.min(value.display_width.saturating_sub(1))
            });
            SelectionPoint { row, column }
        };
        self.anchor = self.anchor.map(|point| clamp(point, &self.rows));
        self.head = self.head.map(|point| clamp(point, &self.rows));
    }
}

impl SelectionRow {
    fn selects_column(&self, column: u16, from: u16, to: u16) -> bool {
        column >= from && column < to && column >= self.gutter_width && column < self.display_width
    }

    fn text_between(&self, from: u16, to: u16) -> String {
        let from = from.max(self.gutter_width);
        let mut output = String::new();
        let mut column = 0_u16;
        for character in self.text.chars() {
            let width = char_width(character);
            let end = column.saturating_add(width);
            if end > from && column < to && end > self.gutter_width {
                output.push(character);
            }
            column = end;
        }
        output
    }
}

pub fn text_width(value: &str) -> u16 {
    value.chars().map(char_width).fold(0, u16::saturating_add)
}

pub fn char_width(value: char) -> u16 {
    ratatui::text::Line::raw(value.to_string())
        .width()
        .max(1)
        .min(u16::MAX as usize) as u16
}
