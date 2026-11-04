use iced::{Background, Border, Element, Theme, border::Radius, widget::container};

use crate::widgets::{
    configured_theme::{background_color, secondary_text_color},
    shared_metrics::{OUTLINE_WIDTH_PX, PRESSABLE_OUTLINE_PADDING, SMALL_CORNER_RADIUS_PX},
};

#[must_use]
pub fn press_icon_chip<'a, Message: 'a>(
    icon: Element<'a, Message>,
    opacity: f32,
) -> Element<'a, Message> {
    container(icon)
        .padding(PRESSABLE_OUTLINE_PADDING)
        .style(move |theme| press_chip_style(theme, opacity))
        .into()
}

fn press_chip_style(theme: &Theme, opacity: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(
            background_color(theme).scale_alpha(opacity),
        )),
        border: Border {
            color: secondary_text_color(theme).scale_alpha(opacity),
            width: OUTLINE_WIDTH_PX,
            radius: Radius::new(SMALL_CORNER_RADIUS_PX),
        },
        ..container::Style::default()
    }
}
