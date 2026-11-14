use corredo::theming::tinted_svg_icon;
use iced::{Color, Element, Theme};
pub use sottomano_phosphor_icons::{Phosphor, Weight};

use crate::widgets::icon_glyph::IconGlyph;

const INK_FRACTION: f32 = 208.0 / 256.0;

#[must_use]
pub fn view_phosphor<'a, Message: 'a>(
    icon: Phosphor,
    ink_size: f32,
    color: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, Message> {
    tinted_svg_icon(icon.svg_data(), ink_size / INK_FRACTION, color)
}

impl From<Phosphor> for IconGlyph {
    fn from(icon: Phosphor) -> Self {
        IconGlyph::Phosphor(icon)
    }
}
