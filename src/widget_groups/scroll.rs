//! Shared scroll-to-step handling for the adjustable bar widgets (audio,
//! brightness, idle timeouts). odin's `ScrollControl` treats a mouse wheel as
//! one step per detent and accumulates finer touchpad travel; this mirrors
//! that with a per-widget pixel accumulator.

use iced::mouse::ScrollDelta;

/// Touchpad pixels of travel per step, so a fine scroll doesn't slew the value;
/// mouse-wheel line events step once each regardless.
const PIXELS_PER_STEP: f32 = 40.0;

/// The step direction from a scroll event: `+1` (up/away) or `-1` (down),
/// `None` for a touchpad nudge that hasn't yet accumulated a full step.
/// `accum` carries the sub-step pixel travel between events and is reset on a
/// step.
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
