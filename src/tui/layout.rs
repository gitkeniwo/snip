use ratatui::layout::Rect;

#[derive(Clone, Debug, Default)]
pub struct LayoutRects {
    pub top_bar: Rect,
    pub bottom_bar: Rect,
    pub sidebar: Rect,
    pub list: Rect,
    pub preview: Rect,
    pub preview_tabs: Rect,
    pub preview_content: Rect,
    pub tab_spans: [(u16, u16); 16],
    pub tab_count: usize,
}

impl LayoutRects {
    pub fn reset_tabs(&mut self) {
        self.tab_spans = [(0, 0); 16];
        self.tab_count = 0;
    }
}

pub fn inner(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

pub fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.right() && row >= area.y && row < area.bottom()
}
