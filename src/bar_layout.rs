//! Bar composition, ported from odin's `WidgetGroup`/`MenuGroup`/`shell.qml`.
//! `top_row`/ `bottom_row` encode the exact top/bottom composition and order
//! from `shell.qml`, calling straight into each group's own state + `view()` —
//! one file per bar section under this module, mirroring odin's
//! one-file-per-`WidgetGroup/*.qml` convention.

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

/// The top bar: battery + the expandable System group (start-anchored), the
/// clock (centered). Groups whose data sources aren't ported yet (Weather,
/// network, media, audio, tray, notifications, quicklaunch, …) are omitted for
/// now rather than shown empty.
pub fn top_row(bar: &Bar, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    let start = Row::with_children(
        [bar.battery_group.view(config), bar.system_group.view(config, position)]
            .into_iter()
            .flatten(),
    )
    .spacing(f32::from(config.layout.horizontal_widget_spacing))
    .align_y(Alignment::Center);
    let center = Row::with_children([bar.clock_group.view(config)])
        .spacing(f32::from(config.layout.horizontal_group_spacing))
        .align_y(Alignment::Center);
    bar_row(start.into(), center.into(), config)
}

/// The bottom bar: the niri workspace pager (start-anchored), the window list
/// (centered).
pub fn bottom_row<'a>(
    bar: &'a Bar,
    output: &'a str,
    position: Position,
) -> Element<'a, Message> {
    let config = &bar.config;
    let start = Row::with_children([workspaces_group::view(
        &bar.niri,
        output,
        config,
        position,
    )])
    .spacing(f32::from(config.layout.horizontal_group_spacing))
    .align_y(Alignment::Center);
    let center = Row::with_children(window_list_group::view(
        &bar.niri,
        output,
        config,
        bar.desktop_entries.as_deref(),
        position,
    ))
    .spacing(f32::from(config.layout.horizontal_group_spacing))
    .align_y(Alignment::Center);
    bar_row(start.into(), center.into(), config)
}

/// Compose the zone content into a strip: start-anchored group, fill, center
/// group, fill — the iced stand-in for odin's per-zone edge anchors. No group
/// is assigned to the end anchor yet.
fn bar_row<'a>(start: Element<'a, Message>, center: Element<'a, Message>, config: &Config) -> Element<'a, Message> {
    row![
        start,
        Space::new().width(Length::Fill),
        center,
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .padding([
        f32::from(config.layout.vertical_padding),
        f32::from(config.layout.horizontal_group_padding),
    ])
    .into()
}