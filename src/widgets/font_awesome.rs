use iced::{Color, Element};

use crate::widgets::{bar_widget::Glyph, svg_icon, tooltip_body::Cell};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontAwesome {
    Solid(Solid),
    Regular(Regular),
    Brands(Brands),
}

impl From<Solid> for FontAwesome {
    fn from(icon: Solid) -> Self {
        FontAwesome::Solid(icon)
    }
}

impl From<Regular> for FontAwesome {
    fn from(icon: Regular) -> Self {
        FontAwesome::Regular(icon)
    }
}

impl From<Brands> for FontAwesome {
    fn from(icon: Brands) -> Self {
        FontAwesome::Brands(icon)
    }
}

impl Solid {
    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        svg_icon::view(self.svg_data(), size, color)
    }
}

impl FontAwesome {
    pub fn from_slug(slug: &str) -> Option<FontAwesome> {
        Solid::from_slug(slug)
            .map(FontAwesome::Solid)
            .or_else(|| Brands::from_slug(slug).map(FontAwesome::Brands))
    }

    const fn svg_data(self) -> &'static [u8] {
        match self {
            FontAwesome::Solid(icon) => icon.svg_data(),
            FontAwesome::Regular(icon) => icon.svg_data(),
            FontAwesome::Brands(icon) => icon.svg_data(),
        }
    }

    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        svg_icon::view(self.svg_data(), size, color)
    }
}

impl From<FontAwesome> for Glyph {
    fn from(icon: FontAwesome) -> Self {
        Glyph::FontAwesome(icon)
    }
}

impl From<Solid> for Glyph {
    fn from(icon: Solid) -> Self {
        Glyph::FontAwesome(icon.into())
    }
}

impl From<Regular> for Glyph {
    fn from(icon: Regular) -> Self {
        Glyph::FontAwesome(icon.into())
    }
}

impl From<Brands> for Glyph {
    fn from(icon: Brands) -> Self {
        Glyph::FontAwesome(icon.into())
    }
}

impl From<FontAwesome> for Cell {
    fn from(icon: FontAwesome) -> Self {
        Cell::Glyph(icon.into())
    }
}
