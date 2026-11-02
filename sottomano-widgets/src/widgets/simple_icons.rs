use iced::{Color, Element, Theme, widget::svg};
pub use sottomano_icon_sets::SimpleIcon;

use crate::widgets::icon_glyph::IconGlyph;

#[must_use]
pub fn view_simple_icon<'a, Message: 'a>(
    icon: SimpleIcon,
    size: f32,
    color: impl Fn(&Theme) -> Color + 'a,
) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(icon.svg_data()))
        .width(size)
        .height(size)
        .style(move |theme, _| svg::Style {
            color: Some(color(theme)),
        })
        .into()
}

impl From<SimpleIcon> for IconGlyph {
    fn from(icon: SimpleIcon) -> Self {
        IconGlyph::SimpleIcon(icon)
    }
}
