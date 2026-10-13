use iced::{Color, Element};

use crate::widgets::{bar_widget::Glyph, svg_icon};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

const INK_FRACTION: f32 = 208.0 / 256.0;

impl Phosphor {
    pub fn view<'a, Message: 'a>(self, ink_size: f32, color: Color) -> Element<'a, Message> {
        svg_icon::view(self.svg_data(), ink_size / INK_FRACTION, color)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
