//! Bar composition, ported from odin's `WidgetGroup`/`MenuGroup`/`shell.qml`. `top_row`/
//! `bottom_row` encode the exact top/bottom composition and order from `shell.qml`,
//! calling straight into each group's own state + `view()` — one file per bar section
//! under this module, mirroring odin's one-file-per-`WidgetGroup/*.qml` convention.

use iced::widget::{Row, Space, row};
use iced::{Alignment, Element, Length};
use niri_ipc::state::EventStreamState;

use crate::bar_program::Message;
use crate::data_sources::app_icon::DesktopEntries;
use crate::layer_surface::Position;
use crate::user_config::Config;

pub mod battery_group;
pub mod clock_group;
pub mod system_group;
pub mod window_list_group;
pub mod workspaces_group;

use battery_group::BatteryGroup;
use clock_group::ClockGroup;
use system_group::SystemGroup;

/// The top bar: battery + the expandable System group (start-anchored), the clock
/// (centered). Groups whose data sources aren't ported yet (Weather, network, media,
/// audio, tray, notifications, quicklaunch, …) are omitted for now rather than shown
/// empty.
pub fn top_row<'a>(
    battery: &BatteryGroup,
    system: &SystemGroup,
    clock: &ClockGroup,
    config: &Config,
    position: Position,
) -> Element<'a, Message> {
    let start = Row::with_children([battery.view(config), system.view(config, position)].into_iter().flatten())
        .spacing(config.layout.horizontal_widget_spacing as f32)
        .align_y(Alignment::Center);
    let center = Row::with_children([clock.view(config)])
        .spacing(config.layout.horizontal_group_spacing as f32)
        .align_y(Alignment::Center);
    bar_row(start.into(), center.into(), config)
}

/// The bottom bar: the niri workspace pager (start-anchored), the window list (centered).
pub fn bottom_row<'a>(
    niri: &EventStreamState,
    output: &str,
    desktop_entries: Option<&DesktopEntries>,
    config: &Config,
    position: Position,
) -> Element<'a, Message> {
    let start = Row::with_children([workspaces_group::view(niri, output, config, position)])
        .spacing(config.layout.horizontal_group_spacing as f32)
        .align_y(Alignment::Center);
    let center = Row::with_children(window_list_group::view(niri, output, config, desktop_entries))
        .spacing(config.layout.horizontal_group_spacing as f32)
        .align_y(Alignment::Center);
    bar_row(start.into(), center.into(), config)
}

/// Compose the zone content into a strip: start-anchored group, fill, center group, fill,
/// end-anchored group — the iced stand-in for odin's per-zone edge anchors. No group is
/// assigned to the end anchor yet.
fn bar_row<'a>(start: Element<'a, Message>, center: Element<'a, Message>, config: &Config) -> Element<'a, Message> {
    row![start, Space::new().width(Length::Fill), center, Space::new().width(Length::Fill), Space::new()]
        .align_y(Alignment::Center)
        .padding([
            config.layout.vertical_padding as f32,
            config.layout.horizontal_group_padding as f32,
        ])
        .into()
}
