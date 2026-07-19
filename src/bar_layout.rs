//! Bar composition and layout.

use iced::{
    Alignment, Element, Length,
    widget::{Row, Space, row},
};

use crate::{
    bar_program::{Bar, Message},
    layer_surface::Position,
    user_config::Config,
};

pub mod battery_group;
pub mod clock_group;
pub mod system_group;
pub mod window_list_group;
pub mod workspaces_group;

pub fn top_row(bar: &Bar, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    let start = Row::with_children(
        [
            bar.battery_group.view(config),
            bar.system_group.view(config, position),
        ]
        .into_iter()
        .flatten(),
    )
    .spacing(config.layout.horizontal_widget_spacing)
    .align_y(Alignment::Center);
    let center = Row::with_children([bar.clock_group.view(config)])
        .spacing(config.layout.horizontal_group_spacing)
        .align_y(Alignment::Center);
    bar_row(start.into(), center.into(), config)
}

pub fn bottom_row<'a>(bar: &'a Bar, output: &'a str, position: Position) -> Element<'a, Message> {
    let config = &bar.config;
    let start = Row::with_children([workspaces_group::view(&bar.niri, output, config, position)])
        .spacing(config.layout.horizontal_group_spacing)
        .align_y(Alignment::Center);
    let center = Row::with_children(window_list_group::view(
        &bar.niri,
        output,
        config,
        bar.desktop_entries.as_deref(),
        position,
    ))
    .spacing(config.layout.horizontal_group_spacing)
    .align_y(Alignment::Center);
    bar_row(start.into(), center.into(), config)
}

fn bar_row<'a>(
    start: Element<'a, Message>,
    center: Element<'a, Message>,
    config: &Config,
) -> Element<'a, Message> {
    row![
        start,
        Space::new().width(Length::Fill),
        center,
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .padding([
        config.layout.vertical_padding,
        config.layout.horizontal_group_padding,
    ])
    .into()
}
