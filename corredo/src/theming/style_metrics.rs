use iced::{Border, Padding, border};

pub(crate) const OUTLINE_WIDTH_PX: f32 = 1.0;
pub(crate) const ACCENT_STROKE_PX: f32 = 2.0;
pub(crate) const SMALL_CORNER_RADIUS_PX: f32 = 2.0;
pub(crate) const PRESSABLE_OUTLINE_PADDING: Padding = Padding {
    top: 1.0,
    right: 4.0,
    bottom: 1.0,
    left: 4.0,
};

#[must_use]
pub(crate) fn pill_corners(thickness: f32) -> Border {
    border::rounded(thickness / 2.0)
}
