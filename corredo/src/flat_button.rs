use iced::{Color, Element, Theme, widget::button};

use crate::theme_colors::text_color;

pub fn flat_button<'a>(
    text_color: impl Fn(&Theme) -> Color + 'a,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'a {
    move |theme, _| button::Style {
        background: None,
        text_color: text_color(theme),
        ..button::Style::default()
    }
}

pub fn flat_element_button<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
) -> Element<'a, Message> {
    button(content)
        .padding(0)
        .style(flat_button(text_color))
        .on_press(on_press)
        .into()
}
