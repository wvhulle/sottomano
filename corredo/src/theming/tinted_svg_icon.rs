use iced::{Color, Element, Length, Theme, widget::svg};

#[must_use]
pub fn tinted_svg_icon<'view, Message: 'view>(
    svg_data: &'static [u8],
    height: f32,
    color: impl Fn(&Theme) -> Color + 'view,
) -> Element<'view, Message> {
    svg(svg::Handle::from_memory(svg_data))
        .width(Length::Shrink)
        .height(height)
        .style(move |theme, _| svg::Style {
            color: Some(color(theme)),
        })
        .into()
}
