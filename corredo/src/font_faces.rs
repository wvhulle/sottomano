use iced::{
    Font,
    font::{Family, Style, Weight},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFaces {
    pub regular: Font,
    pub bold: Font,
    pub italic: Font,
}

impl FontFaces {
    #[must_use]
    pub fn of_family(family: Family) -> Self {
        let regular = Font {
            family,
            ..Font::DEFAULT
        };
        Self {
            regular,
            bold: Font {
                weight: Weight::Bold,
                ..regular
            },
            italic: Font {
                style: Style::Italic,
                ..regular
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextFace {
    pub font: Font,
    pub size_px: f32,
}
