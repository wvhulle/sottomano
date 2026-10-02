use iced::{
    Border, Element, Theme,
    widget::{Container, container},
};

use crate::{config::Palette, state::PanelLayout};

pub fn widget_group_frame<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    layout: &PanelLayout,
    palette: &Palette,
) -> Container<'a, Message> {
    container(content).style(style(layout, palette))
}

pub fn style(layout: &PanelLayout, palette: &Palette) -> impl Fn(&Theme) -> container::Style + 'static {
    let background = layout.frame_background(palette.surface);
    let shadow = layout.frame_shadow();
    let radius = layout.corner_radius;
    let foreground = palette.bar_foreground;
    move |_| container::Style {
        background: Some(background.into()),
        border: Border {
            radius,
            ..Border::default()
        },
        shadow,
        text_color: Some(foreground),
        ..container::Style::default()
    }
}
