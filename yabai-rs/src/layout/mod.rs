pub mod tree;
pub mod tiles;
pub mod accordion;

use crate::wm::Rect;

#[derive(Debug, Clone, Copy)]
pub struct Gaps {
    pub inner_h: i32,
    pub inner_v: i32,
    pub outer_left: i32,
    pub outer_right: i32,
    pub outer_top: i32,
    pub outer_bottom: i32,
}

impl Gaps {
    pub fn shrink(&self, area: Rect) -> Rect {
        Rect {
            x: area.x + self.outer_left,
            y: area.y + self.outer_top,
            width: area.width - self.outer_left - self.outer_right,
            height: area.height - self.outer_top - self.outer_bottom,
        }
    }
}
