//! Bar composition and layout.

use audio_group::AudioMessage;
use battery_group::BatteryMessage;
use iced::{
    Alignment, Element, Length,
    widget::{Row, Space, row},
};
use journal_group::JournalMessage;
use net_interfaces_group::NetInterfacesMessage;
use panel_settings_group::PanelSettingsMessage;
use system_group::SystemMessage;
use workspaces_group::WorkspacesMessage;

use crate::{
    bar_program::{Bar, Message},
    config::resolved::ResolvedConfig,
    layer_surface::Position,
};

pub mod audio_group;
pub mod battery_group;
pub mod clock_group;
pub mod journal_group;
pub mod net_interfaces_group;
pub mod panel_settings_group;
pub mod quick_launch_group;
pub mod source_cell;
pub mod system_group;
pub mod window_list_group;
pub mod workspaces_group;

pub fn top_row(bar: &Bar, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    let start = zone(
        [
            bar.battery_group.view(config).map(|e| {
                e.map(|m| match m {
                    BatteryMessage::Tooltip(t) => Message::Tooltip(t),
                    m @ BatteryMessage::Reading(_) => Message::Battery(m),
                })
            }),
            bar.system_group.view(config, position).map(|e| {
                e.map(|m| match m {
                    SystemMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::System(m),
                })
            }),
            bar.net_interfaces_group.view(config).map(|e| {
                e.map(|m| match m {
                    NetInterfacesMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::NetInterfaces(m),
                })
            }),
            bar.journal_group.view(config).map(|e| {
                e.map(|m| match m {
                    JournalMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Journal(m),
                })
            }),
            bar.panel_settings_group.view(config, position).map(|e| {
                e.map(|m| match m {
                    PanelSettingsMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::PanelSettings(m),
                })
            }),
        ],
        config,
    );
    let center = zone(
        [Some(bar.clock_group.view(config).map(Message::Clock))],
        config,
    );
    let end = zone(
        [bar.audio_group.view(config).map(|e| {
            e.map(|m| match m {
                AudioMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Audio(m),
            })
        })],
        config,
    );
    bar_row(start, center, end, config)
}

/// A bar zone: the groups that want to show (the `Some` ones), spaced by the
/// configured group spacing. Hidden groups (`None`) contribute no slot, so they
/// never widen the gap between their visible neighbours.
fn zone<'a>(
    groups: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    config: &ResolvedConfig,
) -> Element<'a, Message> {
    Row::with_children(groups.into_iter().flatten())
        .spacing(config.layout.horizontal_group_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

pub fn bottom_row<'a>(bar: &'a Bar, output: &'a str, position: Position) -> Element<'a, Message> {
    let config = &bar.config;
    let start = zone(
        [Some(
            workspaces_group::view(&bar.niri_snapshot, output, config, position)
                .map(|WorkspacesMessage::Clicked(id)| Message::WorkspaceClicked(id)),
        )],
        config,
    );
    let quick_launch = bar
        .quick_launch
        .view(output, config, position)
        .map(|e| e.map(Message::QuickLaunch));
    let window_list = bar
        .window_list
        .view(&bar.niri_snapshot, output, config, position)
        .map(|e| e.map(Message::WindowList));
    let center = Row::new()
        .extend(quick_launch)
        .extend(window_list)
        .spacing(config.layout.horizontal_group_spacing_px)
        .align_y(Alignment::Center);
    // No bottom-right groups ported yet (odin's tray / ai-usage / notifications).
    let end = Element::from(Space::new());
    bar_row(start, center.into(), end, config)
}

fn bar_row<'a>(
    start: Element<'a, Message>,
    center: Element<'a, Message>,
    end: Element<'a, Message>,
    config: &ResolvedConfig,
) -> Element<'a, Message> {
    row![
        start,
        Space::new().width(Length::Fill),
        center,
        Space::new().width(Length::Fill),
        end,
    ]
    .align_y(Alignment::Center)
    .padding([
        config.layout.vertical_padding_px,
        config.layout.horizontal_group_padding_px,
    ])
    .into()
}
