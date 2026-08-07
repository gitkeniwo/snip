use ratatui::layout::Rect;

use super::preview::PreviewTarget;

#[derive(Clone, Debug, Default)]
pub struct LayoutRects {
    pub top_bar: Rect,
    pub bottom_bar: Rect,
    pub sidebar: Rect,
    pub list: Rect,
    pub preview: Rect,
    pub preview_fragments: Rect,
    pub fragment_rows: Vec<(u16, PreviewTarget)>,
    pub preview_content: Rect,
}

impl LayoutRects {
    pub fn reset_fragment_rows(&mut self) {
        self.fragment_rows.clear();
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
