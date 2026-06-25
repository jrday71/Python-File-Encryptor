use super::Gaps;
use crate::wm::Rect;
use crate::layout::tree::{NodeKind, SplitDirection, Tree};

pub struct TilesLayout<'a> {
    pub tree: &'a Tree,
}

impl<'a> TilesLayout<'a> {
    pub fn compute(&self, area: Rect, gaps: &Gaps) -> Vec<(u32, Rect)> {
        let inner = gaps.shrink(area);
        let mut result = vec![];
        if let Some(root) = self.tree.root() {
            self.layout_node(root, inner, gaps, &mut result);
        }
        result
    }

    fn layout_node(&self, id: u64, area: Rect, gaps: &Gaps, out: &mut Vec<(u32, Rect)>) {
        let node = match self.tree.get(id) {
            Some(n) => n,
            None => return,
        };
        match &node.kind {
            NodeKind::Window { window_id } => {
                out.push((*window_id, area));
            }
            NodeKind::Container { split, children } => {
                let total_ratio: f32 = children.iter()
                    .filter_map(|&c| self.tree.get(c))
                    .map(|n| n.ratio)
                    .sum();

                let child_count = children.len();
                let mut cursor = match split {
                    SplitDirection::Horizontal => area.x,
                    SplitDirection::Vertical => area.y,
                };

                for (i, &child_id) in children.iter().enumerate() {
                    let child_node = match self.tree.get(child_id) {
                        Some(n) => n,
                        None => continue,
                    };
                    let fraction = child_node.ratio / total_ratio;
                    let is_last = i + 1 == child_count;

                    let child_rect = match split {
                        SplitDirection::Horizontal => {
                            let w = if is_last {
                                area.x + area.width - cursor
                            } else {
                                (fraction * area.width as f32) as i32
                            };
                            let gap_right = if is_last { 0 } else { gaps.inner_h / 2 };
                            let gap_left = if i == 0 { 0 } else { gaps.inner_h / 2 };
                            let r = Rect { x: cursor + gap_left, y: area.y, width: w - gap_left - gap_right, height: area.height };
                            cursor += w;
                            r
                        }
                        SplitDirection::Vertical => {
                            let h = if is_last {
                                area.y + area.height - cursor
                            } else {
                                (fraction * area.height as f32) as i32
                            };
                            let gap_bottom = if is_last { 0 } else { gaps.inner_v / 2 };
                            let gap_top = if i == 0 { 0 } else { gaps.inner_v / 2 };
                            let r = Rect { x: area.x, y: cursor + gap_top, width: area.width, height: h - gap_top - gap_bottom };
                            cursor += h;
                            r
                        }
                    };
                    self.layout_node(child_id, child_rect, gaps, out);
                }
            }
        }
    }
}
