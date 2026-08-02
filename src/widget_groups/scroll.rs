use iced::mouse::ScrollDelta;

const PIXELS_PER_STEP: f32 = 40.0;

pub(crate) fn scroll_step(accum: &mut f32, delta: ScrollDelta) -> Option<i64> {
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
            *accum += y;
            if accum.abs() >= PIXELS_PER_STEP {
                let step = if *accum > 0.0 { 1 } else { -1 };
                *accum = 0.0;
                Some(step)
            } else {
                None
            }
        }
    }
}
