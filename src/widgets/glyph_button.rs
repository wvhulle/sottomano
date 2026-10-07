use iced::{Color, Element, widget::button};

use crate::widgets::{bar_widget::Glyph, flat_button};

pub struct GlyphButton<Message> {
    glyph: Glyph,
    size: f32,
    color: Color,
    on_press: Message,
}

impl<Message> GlyphButton<Message> {
    pub fn new(glyph: impl Into<Glyph>, size: f32, color: Color, on_press: Message) -> Self {
        Self {
            glyph: glyph.into(),
            size,
            color,
            on_press,
        }
    }
}

impl<'a, Message: 'a + Clone> From<GlyphButton<Message>> for Element<'a, Message> {
    fn from(glyph_button: GlyphButton<Message>) -> Self {
        button(
            glyph_button
                .glyph
                .view(glyph_button.size, glyph_button.color),
        )
        .padding(0)
        .style(flat_button(glyph_button.color))
        .on_press(glyph_button.on_press)
        .into()
    }
}
