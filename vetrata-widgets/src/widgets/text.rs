use iced::{
    Color, Font,
    advanced::{graphics::text::font_system, text as advanced_text},
    font::{Family, Style, Weight},
    widget::{
        Text, span, text,
        text::{IntoFragment, Shaping, Span},
    },
};
use vetrata_config::theme::{
    Theme,
    font::{FontRole, ResolvedFont},
};

pub const LINE_HEIGHT: advanced_text::LineHeight = advanced_text::LineHeight::Relative(1.1);

pub fn column_heading<'a>(
    font: &ResolvedFont,
    title: impl IntoFragment<'a>,
    color: Color,
) -> Text<'a, iced::Theme, iced::Renderer> {
    bar_text(font, title)
        .font(FontFaces::from(font).italic)
        .color(color)
}

pub fn bar_text<'a>(
    font: &ResolvedFont,
    content: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    text(content)
        .size(font.text_size_px)
        .font(FontFaces::from(font).regular)
        .line_height(LINE_HEIGHT)
        .shaping(Shaping::Advanced)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFaces {
    pub regular: Font,
    pub bold: Font,
    pub italic: Font,
}

impl From<&ResolvedFont> for FontFaces {
    fn from(font: &ResolvedFont) -> Self {
        let regular = Font {
            family: match font.role {
                FontRole::Text => Family::SansSerif,
                FontRole::Monospace => Family::Monospace,
            },
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

impl FontFaces {
    pub fn point_generic_families_at(theme: &Theme) {
        let mut fonts = font_system().write().expect("font system lock poisoned");
        let database = fonts.raw().db_mut();
        database.set_sans_serif_family(theme.font.family.clone());
        database.set_monospace_family(theme.monospace_font.family.clone());
    }
}

#[must_use]
pub fn colored_span<'a>(text: String, font: Font, color: Color) -> Span<'a> {
    span(text).font(font).color(color)
}
