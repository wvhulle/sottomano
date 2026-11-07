use iced::{
    Font,
    font::{Family, Style, Weight},
    widget::{
        Text, text,
        text::{IntoFragment, Shaping},
    },
};

pub use crate::shared_metrics::LINE_HEIGHT;

const DEFAULT_TEXT_SIZE_PX: f32 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFaces {
    pub regular: Font,
    pub bold: Font,
    pub italic: Font,
}

impl From<Family> for FontFaces {
    fn from(family: Family) -> Self {
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

impl Default for TextFace {
    fn default() -> Self {
        Self {
            font: Font::DEFAULT,
            size_px: DEFAULT_TEXT_SIZE_PX,
        }
    }
}

impl TextFace {
    pub fn text<'a>(self, content: impl IntoFragment<'a>) -> Text<'a, iced::Theme, iced::Renderer> {
        text(content)
            .size(self.size_px)
            .font(self.font)
            .line_height(LINE_HEIGHT)
            .shaping(Shaping::Advanced)
    }
}
