use super::Gaps;
use crate::wm::Rect;

/// Stacked accordion: windows are overlaid with a visible offset so titles show.
pub struct AccordionLayout {
    pub padding: i32,
}

impl AccordionLayout {
    pub fn compute(&self, window_ids: &[u32], area: Rect, gaps: &Gaps, focused: Option<u32>) -> Vec<(u32, Rect)> {
        let inner = gaps.shrink(area);
        let count = window_ids.len() as i32;
        if count == 0 {
            return vec![];
        }

        let focused_idx = focused
            .and_then(|fid| window_ids.iter().position(|&w| w == fid))
            .unwrap_or(0) as i32;

        window_ids.iter().enumerate().map(|(i, &wid)| {
            let i = i as i32;
            let offset_x = i * self.padding;
            let offset_y = i * self.padding;
            let shrink_w = (count - 1) * self.padding;
            let shrink_h = (count - 1) * self.padding;

            let rect = if i == focused_idx {
                // focused window fills most of the space
                Rect {
                    x: inner.x + offset_x,
                    y: inner.y + offset_y,
                    width: inner.width - shrink_w,
                    height: inner.height - shrink_h,
                }
            } else {
                // background window just peeks
                Rect {
                    x: inner.x + offset_x,
                    y: inner.y + offset_y,
                    width: inner.width - shrink_w,
                    height: inner.height - shrink_h,
                }
            };
            (wid, rect)
        }).collect()
    }
}
