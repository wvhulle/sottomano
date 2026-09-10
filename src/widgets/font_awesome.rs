use iced::{
    Font, Theme,
    font::{Family, Weight},
    widget::{Text, text, text::LineHeight},
};
use iced_font_awesome::{Icon as FaIcon, fa_icon_solid};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

const FREE: Family = Family::Name("Font Awesome 7 Free");
const BRANDS: Family = Family::Name("Font Awesome 7 Brands");

const SOLID_FONT: Font = Font {
    family: FREE,
    weight: Weight::Black,
    ..Font::DEFAULT
};

const REGULAR_FONT: Font = Font {
    family: FREE,
    ..Font::DEFAULT
};

const BRANDS_FONT: Font = Font {
    family: BRANDS,
    ..Font::DEFAULT
};

pub fn load_fonts() {
    let _: FaIcon<'_, Theme> = fa_icon_solid(Solid::Music.name());
}

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

fn glyph<'a>(code: char, font: Font) -> Text<'a> {
    text(code).font(font).line_height(LineHeight::Relative(1.0))
}

impl Solid {
    pub fn glyph<'a>(self) -> Text<'a> {
        glyph(self.code(), SOLID_FONT)
    }
}

impl Regular {
    pub fn glyph<'a>(self) -> Text<'a> {
        glyph(self.code(), REGULAR_FONT)
    }
}

impl Brands {
    pub fn glyph<'a>(self) -> Text<'a> {
        glyph(self.code(), BRANDS_FONT)
    }
}

impl FontAwesome {
    pub fn from_slug(slug: &str) -> Option<FontAwesome> {
        Solid::from_slug(slug)
            .map(FontAwesome::Solid)
            .or_else(|| Brands::from_slug(slug).map(FontAwesome::Brands))
    }

    pub fn glyph<'a>(self) -> Text<'a> {
        match self {
            FontAwesome::Solid(icon) => icon.glyph(),
            FontAwesome::Regular(icon) => icon.glyph(),
            FontAwesome::Brands(icon) => icon.glyph(),
        }
    }
}
