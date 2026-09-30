use iced::{
    Color, Element, Length,
    widget::{Space, container},
};
use iced_exwlshell::reexport::{Layer, NewLayerShellSettings};

use crate::{
    app::Message,
    wayland_layers::{
        niri_rules::StyledSurface,
        surface::{self, Margin, Position},
    },
};

pub fn margin(position: Position, height: u32) -> Margin {
    let offscreen = -height.cast_signed();
    match position {
        Position::Top => Margin {
            top: offscreen,
            ..Margin::default()
        },
        Position::Bottom => Margin {
            bottom: offscreen,
            ..Margin::default()
        },
        Position::Left => Margin {
            left: offscreen,
            ..Margin::default()
        },
    }
}

pub fn settings(output: &str, position: Position, height: u32) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: position.strip_size(height),
        exclusive_zone: Some(-1),
        anchor: position.anchor(),
        layer: Layer::Background,
        margin: Some(margin(position, height)),
        events_transparent: true,
        ..surface::base_settings(Some(output), StyledSurface::Backdrop(position).namespace())
    }
}

pub fn fill<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: Some(Color::BLACK.into()),
            ..Default::default()
        })
        .into()
}
