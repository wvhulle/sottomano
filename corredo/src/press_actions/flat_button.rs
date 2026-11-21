use iced::{Color, Element, Theme, widget::button};

use crate::theming::text_color;

pub fn flat_button<'view>(
    text_color: impl Fn(&Theme) -> Color + 'view,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'view {
    move |theme, _| button::Style {
        background: None,
        text_color: text_color(theme),
        ..button::Style::default()
    }
}

pub fn flat_element_button<'view, Message: Clone + 'view>(
    content: impl Into<Element<'view, Message>>,
    on_press: Message,
) -> Element<'view, Message> {
    button(content)
        .padding(0)
        .style(flat_button(text_color))
        .on_press(on_press)
        .into()
}
