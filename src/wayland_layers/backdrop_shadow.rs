use iced::{
    Color, Element, Length,
    widget::{Space, container},
};
use iced_layershell::reexport::{
    KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{app::Message, wayland_layers::surface::Position};

pub fn namespace(position: Position) -> &'static str {
    match position {
        Position::Top => "vetrata-backdrop-top",
        Position::Bottom => "vetrata-backdrop-bottom",
    }
}

pub fn settings(output: &str, position: Position, height: u32) -> NewLayerShellSettings {
    let h = height.cast_signed();
    let margin = match position {
        Position::Top => (-h, 0, 0, 0),
        Position::Bottom => (0, 0, -h, 0),
    };
    NewLayerShellSettings {
        size: Some((0, height)),
        exclusive_zone: Some(-1),
        anchor: position.anchor(),
        layer: Layer::Background,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(margin),
        events_transparent: true,
        namespace: Some(namespace(position).to_string()),
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
