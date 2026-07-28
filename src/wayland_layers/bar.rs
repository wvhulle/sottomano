//! The bar-strip layer: the full-width panel drawn on each output/edge — its
//! chrome and click-to-dismiss overlay, and the composition of the
//! [`widget_groups`](crate::widget_groups) into its top and bottom rows — plus
//! the off-screen backdrop fill whose silhouette casts the edge shadow. The
//! surface *settings* for both live in [`surface`](super::surface); this module
//! is what those surfaces actually draw.

use iced::{
    Alignment, Element, Length,
    widget::{Row, Space, container, mouse_area, row, stack, text},
    window::Id,
};
use iced_layershell::reexport::{
    KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    app::{Bar, Message},
    config::resolved::ResolvedConfig,
    wayland_layers::{backdrop_shadow, surface::Position, tooltip},
    widget_groups::{
        audio_group::AudioMessage,
        battery_group::BatteryMessage,
        idle_timeouts_group::IdleTimeoutsMessage,
        internet_group::InternetMessage,
        journal_group::JournalMessage,
        media_group::MediaMessage,
        monitor_group::MonitorMessage,
        net_interfaces_group::NetInterfacesMessage,
        panel_settings_group::PanelSettingsMessage,
        system_group::SystemMessage,
        tray_group::TrayMessage,
        weather_group::WeatherMessage,
        workspaces_group::{self, WorkspacesMessage},
    },
};

/// Render the bar surface `window` draws: a top/bottom strip of widget groups
/// wrapped in the bar chrome, or — for the off-screen shadow caster — the
/// opaque backdrop fill. An unknown window (already closed) draws nothing.
pub fn view(bar: &Bar, window: Id) -> Element<'_, Message> {
    if bar.backdrops.contains(&window) {
        return backdrop_shadow::fill();
    }
    let Some(surface) = bar.surfaces.get(&window) else {
        return container(text("")).into();
    };
    let content = match surface.position {
        Position::Top => top_row(bar, surface.position),
        Position::Bottom => bottom_row(bar, &surface.output, surface.position),
    };
    // Stamp this surface onto tooltip hovers bubbling out of the bar, so leaf
    // groups need not carry it.
    chrome(bar, tooltip::on_surface(window, content))
}

/// Margin (top, right, bottom, left) that insets the bar from the screen edges
/// in floating mode — `0` on every side flush-mounts it instead.
pub fn float_margin(position: Position, offset_px: i32) -> (i32, i32, i32, i32) {
    match position {
        Position::Top => (offset_px, offset_px, 0, offset_px),
        Position::Bottom => (0, offset_px, offset_px, offset_px),
    }
}

/// Layer-shell settings for one bar strip. It reserves its own strip via an
/// exclusive zone equal to its height (odin's `exclusiveZone: barOuterHeight`),
/// so niri tiles windows clear of it instead of letting them overlap. `height`
/// is odin's `barOuterHeight` from the shared config; the two bars stack below
/// odin's own reserved zone so they sit flush without overlapping windows.
/// `margin` is odin's floating inset — `(0, 0, 0, 0)` when flush.
pub fn layer_settings(
    output: &str,
    position: Position,
    height: u32,
    margin: (i32, i32, i32, i32),
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, height)),
        exclusive_zone: Some(height as i32),
        anchor: position.anchor(),
        layer: Layer::Top,
        // Never take keyboard focus — mirrors Odin's WlrKeyboardFocus.None.
        keyboard_interactivity: KeyboardInteractivity::None,
        // Bind this surface to a specific niri output by name.
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(margin),
        ..Default::default()
    }
}

/// Wrap the bar content in the strip chrome: a full-bleed click-to-dismiss
/// layer behind it, and the rounded, tinted panel background.
fn chrome<'a>(bar: &'a Bar, content: Element<'a, Message>) -> Element<'a, Message> {
    let dismiss = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::DismissPressed);

    container(stack![dismiss, content])
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            background: Some(bar.config.palette.bar_background.into()),
            border: iced::Border {
                radius: bar.config.chrome_radius().into(),
                ..Default::default()
            },
            text_color: Some(bar.config.palette.bar_foreground),
            ..Default::default()
        })
        .into()
}

fn top_row(bar: &Bar, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    let start = zone(
        [
            bar.weather_group.view(config, position).map(|e| {
                e.map(|m| match m {
                    WeatherMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Weather(m),
                })
            }),
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
            bar.internet_group.view(config, position).map(|e| {
                e.map(|m| match m {
                    InternetMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Internet(m),
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
        [
            bar.monitor_group.view(config, position).map(|e| {
                e.map(|m| match m {
                    MonitorMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Monitor(m),
                })
            }),
            Some(
                bar.idle_timeouts_group
                    .view(config, position)
                    .map(|m| match m {
                        IdleTimeoutsMessage::Tooltip(t) => Message::Tooltip(t),
                        m => Message::IdleTimeouts(m),
                    }),
            ),
            bar.media_group.view(config, position).map(|e| {
                e.map(|m| match m {
                    MediaMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Media(m),
                })
            }),
            bar.audio_group.view(config).map(|e| {
                e.map(|m| match m {
                    AudioMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Audio(m),
                })
            }),
        ],
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

fn bottom_row<'a>(bar: &'a Bar, output: &'a str, position: Position) -> Element<'a, Message> {
    let config = &bar.config;
    let start = zone(
        [Some(
            workspaces_group::view(&bar.niri_snapshot, output, config, position)
                .map(|WorkspacesMessage::Clicked(id)| Message::WorkspaceClicked(id)),
        )],
        config,
    );
    // Launcher + window list, centred as a pair in the Fill middle by the window
    // group. Shrink flanks (workspaces, tray) lay out first, so the middle
    // reserves exactly the space between them; the window list caps its width to
    // that budget and scrolls when it overflows, keeping the tray from being
    // overrun.
    let middle = container(
        bar.window_group
            .view(&bar.niri_snapshot, output, config, position)
            .map(Message::WindowGroup),
    )
    .center_x(Length::Fill);
    // Bottom-right zone (odin: tray / ai-usage / notifications). Tray ported.
    let end = bar
        .tray_group
        .view(config, position)
        .map(|e| {
            e.map(|m| match m {
                TrayMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Tray(m),
            })
        })
        .unwrap_or_else(|| Space::new().into());
    row![start, middle, end]
        .align_y(Alignment::Center)
        .padding([
            config.layout.vertical_padding_px,
            config.layout.horizontal_group_padding_px,
        ])
        .into()
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
