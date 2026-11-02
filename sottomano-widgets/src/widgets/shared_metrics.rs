use iced::{Border, border};

pub const OUTLINE_WIDTH_PX: f32 = 1.0;
pub const ACCENT_STROKE_PX: f32 = 2.0;
pub const SMALL_CORNER_RADIUS_PX: f32 = 2.0;
pub const SCROLL_LINE_STEP_PX: f32 = 40.0;

#[must_use]
pub fn pill_corners(thickness: f32) -> Border {
    border::rounded(thickness / 2.0)
}
