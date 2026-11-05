use iced::mouse::ScrollDelta;

use crate::shared_metrics::SCROLL_LINE_STEP_PX;

pub fn scroll_direction(accumulated_px: &mut f32, delta: ScrollDelta) -> Option<i64> {
    match delta {
        ScrollDelta::Lines { y, .. } => {
            if y > 0.0 {
                Some(1)
            } else if y < 0.0 {
                Some(-1)
            } else {
                None
            }
        }
        ScrollDelta::Pixels { y, .. } => {
            *accumulated_px += y;
            if accumulated_px.abs() >= SCROLL_LINE_STEP_PX {
                let step = if *accumulated_px > 0.0 { 1 } else { -1 };
                *accumulated_px = 0.0;
                Some(step)
            } else {
                None
            }
        }
    }
}
