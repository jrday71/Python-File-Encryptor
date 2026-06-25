use anyhow::Result;
use crate::wm::Rect;

#[derive(Debug, Clone)]
pub struct Display {
    pub id: u32,
    pub frame: Rect,
    pub is_main: bool,
}

impl Display {
    pub fn list() -> Result<Vec<Display>> {
        #[cfg(target_os = "macos")]
        {
            macos::list_displays()
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Fallback: pretend we have one 1920×1080 display
            Ok(vec![Display {
                id: 1,
                frame: Rect { x: 0, y: 0, width: 1920, height: 1080 },
                is_main: true,
            }])
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use core_graphics::display::CGDisplay;

    pub fn list_displays() -> Result<Vec<Display>> {
        let displays = CGDisplay::active_displays()
            .map_err(|e| anyhow::anyhow!("CGDisplay::active_displays failed: {e:?}"))?;

        let result = displays.iter().enumerate().map(|(i, &did)| {
            let d = CGDisplay::new(did);
            let bounds = d.bounds();
            Display {
                id: did,
                frame: Rect {
                    x: bounds.origin.x as i32,
                    y: bounds.origin.y as i32,
                    width: bounds.size.width as i32,
                    height: bounds.size.height as i32,
                },
                is_main: i == 0,
            }
        }).collect();
        Ok(result)
    }
}
