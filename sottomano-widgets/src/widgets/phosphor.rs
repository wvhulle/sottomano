use iced::{Color, Element, Theme};
pub use sottomano_icon_sets::{Phosphor, Weight};

use crate::widgets::{bar_widget::Glyph, svg_icon};

const INK_FRACTION: f32 = 208.0 / 256.0;

#[must_use]
pub fn view_phosphor<'a, Message: 'a>(
    icon: Phosphor,
    ink_size: f32,
    color: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, Message> {
    svg_icon::view(icon.svg_data(), ink_size / INK_FRACTION, color)
}

impl From<Phosphor> for Glyph {
    fn from(icon: Phosphor) -> Self {
        Glyph::Phosphor(icon)
    }
}
