use iced::{Color, Element, Length, widget::svg};

pub const PHOSPHOR_INK_FRACTION: f32 = 208.0 / 256.0;

pub fn view<'a, Message: 'a>(
    svg_data: &'static [u8],
    height: f32,
    color: Color,
) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(svg_data))
        .width(Length::Shrink)
        .height(height)
        .style(move |_, _| svg::Style { color: Some(color) })
        .into()
}
