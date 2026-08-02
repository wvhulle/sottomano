use iced::widget::text::Catalog;
use iced_font_awesome::{Icon as FaIcon, fa_icon, fa_icon_brands, fa_icon_solid};

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
    pub fn widget<'a, Theme: Catalog>(self) -> FaIcon<'a, Theme> {
        fa_icon_solid(self.name())
    }
}

impl Regular {
    pub fn widget<'a, Theme: Catalog>(self) -> FaIcon<'a, Theme> {
        fa_icon(self.name())
    }
}

impl Brands {
    pub fn widget<'a, Theme: Catalog>(self) -> FaIcon<'a, Theme> {
        fa_icon_brands(self.name())
    }
}

impl FontAwesome {
    pub fn from_slug(slug: &str) -> Option<FontAwesome> {
        Solid::from_slug(slug)
            .map(FontAwesome::Solid)
            .or_else(|| Brands::from_slug(slug).map(FontAwesome::Brands))
    }

    pub fn widget<'a, Theme: Catalog>(self) -> FaIcon<'a, Theme> {
        match self {
            FontAwesome::Solid(icon) => icon.widget(),
            FontAwesome::Regular(icon) => icon.widget(),
            FontAwesome::Brands(icon) => icon.widget(),
        }
    }
}
