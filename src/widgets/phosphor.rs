use crate::widgets::bar_widget::Glyph;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub const PHOSPHOR_INK_FRACTION: f32 = 208.0 / 256.0;

pub fn ink_side(icon_side: f32) -> f32 {
    icon_side * PHOSPHOR_INK_FRACTION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Thin,
    Light,
    Regular,
    Bold,
    Fill,
    Duotone,
}

impl From<Phosphor> for Glyph {
    fn from(icon: Phosphor) -> Self {
        Glyph::Phosphor(icon)
    }
}
