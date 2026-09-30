use iced::{
    Color, Element, Length,
    widget::{Space, container},
};
use iced_exwlshell::reexport::{Layer, NewLayerShellSettings};

use crate::{
    app::Message,
    wayland_layers::surface::{self, Position},
};

pub fn namespace(position: Position) -> &'static str {
    match position {
        Position::Top => "vetrata-backdrop-top",
        Position::Bottom => "vetrata-backdrop-bottom",
        Position::Left => "vetrata-backdrop-left",
    }
}

pub fn margin(position: Position, height: u32) -> (i32, i32, i32, i32) {
    let h = height.cast_signed();
    match position {
        Position::Top => (-h, 0, 0, 0),
        Position::Bottom => (0, 0, -h, 0),
        Position::Left => (0, 0, 0, -h),
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
        ..surface::base_settings(Some(output), namespace(position))
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
