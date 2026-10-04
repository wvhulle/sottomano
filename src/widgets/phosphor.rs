use crate::widgets::bar_widget::Glyph;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

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
