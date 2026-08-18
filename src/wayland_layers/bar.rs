use core::f32::consts::FRAC_1_SQRT_2;

use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, Space, container, mouse_area, row, stack, text},
    window::Id,
};
use iced_layershell::reexport::{
    KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    app::{Bar, Message},
    blend,
    config::Config,
    state::ChromeState,
    wayland_layers::{
        backdrop_shadow,
        blur_caster::{self, Zone},
        surface::{Margin, Position},
        tooltip,
    },
    widget_groups::{
        ai_group::AiMessage,
        audio_group::AudioMessage,
        battery_group::BatteryMessage,
        color_temperature_group::ColorTemperatureMessage,
        idle_timeouts_group::IdleTimeoutsMessage,
        internet_group::InternetMessage,
        journal_group::JournalMessage,
        media_group::MediaMessage,
        monitor_group::MonitorMessage,
        net_interfaces_group::NetInterfacesMessage,
        notification_group,
        panel_settings_group::PanelSettingsMessage,
        power_profile_group::PowerProfileMessage,
        system_group::SystemMessage,
        tray_group::TrayMessage,
        weather_group::WeatherMessage,
        window_group::{WindowGroupMessage, window_list::WindowListMessage},
        workspaces_group::{self, WorkspacesMessage},
    },
    widgets::bounds_probe::bounds_probe,
};

pub fn view(bar: &Bar, window: Id) -> Element<'_, Message> {
    if bar.backdrops.contains(&window) {
        return backdrop_shadow::fill();
    }
    if bar.blur_casters.contains(window) {
        return blur_caster::fill();
    }
    let Some(surface) = bar.surfaces.get(&window) else {
        return container(text("")).into();
    };
    let content = match surface.position {
        Position::Top => top_row(bar, window, surface.position),
        Position::Bottom => bottom_row(bar, window, &surface.output, surface.position),
    };
    chrome(bar, tooltip::on_surface(window, content))
}

pub fn float_margin(position: Position, offset_px: i32) -> Margin {
    match position {
        Position::Top => Margin {
            top: offset_px,
            right: offset_px,
            bottom: 0,
            left: offset_px,
        },
        Position::Bottom => Margin {
            top: 0,
            right: offset_px,
            bottom: offset_px,
            left: offset_px,
        },
    }
}

pub fn layer_settings(
    output: &str,
    position: Position,
    height: u32,
    margin: Margin,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((0, height)),
        exclusive_zone: Some(height.cast_signed()),
        anchor: position.anchor(),
        layer: Layer::Top,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(margin.edges()),
        ..Default::default()
    }
}

fn chrome<'a>(bar: &'a Bar, content: Element<'a, Message>) -> Element<'a, Message> {
    let dismiss = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::DismissPressed);

    let centered = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Vertical::Center);

    container(stack![dismiss, centered])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style {
            background: (!bar.session.chrome.disconnected_groups).then(|| {
                blend::darkened(
                    bar.config.palette.bar_background,
                    bar.session.chrome.darken_chrome_factor,
                )
                .into()
            }),
            border: iced::Border {
                radius: bar.session.chrome.radius().into(),
                ..Default::default()
            },
            text_color: Some(bar.config.palette.bar_foreground),
            ..Default::default()
        })
        .into()
}

fn top_start(bar: &Bar, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    zone(
        [
            slot(bar.weather_group.view(config, position), |m| match m {
                WeatherMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Weather(m),
            }),
            slot(bar.battery_group.view(config, position), |m| match m {
                BatteryMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Battery(m),
            }),
            slot(
                bar.power_profile_group.view(config, position),
                |m| match m {
                    PowerProfileMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::PowerProfile(m),
                },
            ),
            slot(bar.system_group.view(config, position), |m| match m {
                SystemMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::System(m),
            }),
            slot(
                bar.net_interfaces_group.view(config, position),
                |m| match m {
                    NetInterfacesMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::NetInterfaces(m),
                },
            ),
            slot(bar.journal_group.view(config, position), |m| match m {
                JournalMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Journal(m),
            }),
            slot(bar.internet_group.view(config, position), |m| match m {
                InternetMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Internet(m),
            }),
            slot(
                bar.panel_settings_group
                    .view(config, &bar.session.chrome, position),
                |m| match m {
                    PanelSettingsMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::PanelSettings(m),
                },
            ),
        ],
        config,
        &bar.session.chrome,
    )
}

fn top_row(bar: &Bar, window: Id, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    let chrome = &bar.session.chrome;
    let start = measured(top_start(bar, position), chrome, window, Zone::TopStart);
    let center = zone(
        [
            slot(bar.clock_group.view(config, position), Message::Clock),
            slot(
                notification_group::view(
                    config,
                    &bar.session.notifications,
                    position,
                    bar.notifications.is_active(),
                ),
                Message::NotificationGroup,
            ),
        ],
        config,
        chrome,
    );
    let center = measured(center, chrome, window, Zone::TopCenter);
    let end = zone(
        [
            slot(bar.ai_group.view(config, position), |m| match m {
                AiMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Ai(m),
            }),
            slot(bar.monitor_group.view(config, position), |m| match m {
                MonitorMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Monitor(m),
            }),
            slot(
                bar.color_temperature_group.view(config, position),
                |m| match m {
                    ColorTemperatureMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::ColorTemperature(m),
                },
            ),
            slot(
                bar.idle_timeouts_group.view(config, position),
                |m| match m {
                    IdleTimeoutsMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::IdleTimeouts(m),
                },
            ),
            slot(
                bar.media_group.view(config, &bar.desktop_entries, position),
                |m| match m {
                    MediaMessage::Tooltip(t) => Message::Tooltip(t),
                    m => Message::Media(m),
                },
            ),
            slot(bar.audio_group.view(config, position), |m| match m {
                AudioMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Audio(m),
            }),
        ],
        config,
        chrome,
    );
    let end = measured(end, chrome, window, Zone::TopEnd);
    bar_row(start, center, end, config)
}

fn measured<'a>(
    content: Element<'a, Message>,
    chrome: &ChromeState,
    window: Id,
    zone: Zone,
) -> Element<'a, Message> {
    if !chrome.disconnected_groups {
        return content;
    }
    bounds_probe(content, move |rects| Message::PillBounds {
        window,
        zone,
        rects,
    })
}

fn zone<'a>(
    groups: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    config: &'a Config,
    chrome: &ChromeState,
) -> Element<'a, Message> {
    let spacing = if chrome.disconnected_groups {
        config.spacing.widget_spacing_px
    } else {
        config.spacing.group_spacing_px
    };
    Row::with_children(
        groups
            .into_iter()
            .flatten()
            .map(|group| pill(group, config, chrome)),
    )
    .spacing(spacing)
    .align_y(Alignment::Center)
    .into()
}

fn pill<'a>(
    group: Element<'a, Message>,
    config: &Config,
    chrome: &ChromeState,
) -> Element<'a, Message> {
    if !chrome.disconnected_groups {
        return group;
    }
    let background = blend::darkened(config.palette.bar_background, chrome.darken_chrome_factor);
    let radius = chrome.radius();
    let curve_inset = radius * (1.0 - FRAC_1_SQRT_2);
    container(group)
        .height(Length::Fixed(config.chrome.outer_height_px))
        .align_y(Vertical::Center)
        .padding([0.0, curve_inset + config.spacing.group_padding_px])
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius: radius.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn slot<'a, M: 'a>(
    element: impl Into<Option<Element<'a, M>>>,
    wrap: impl Fn(M) -> Message + 'a,
) -> Option<Element<'a, Message>> {
    element.into().map(|e| e.map(wrap))
}

fn bottom_row<'a>(
    bar: &'a Bar,
    window: Id,
    output: &'a str,
    position: Position,
) -> Element<'a, Message> {
    let config = &bar.config;
    let chrome = &bar.session.chrome;
    let start = zone(
        [slot(
            workspaces_group::view(&bar.niri_snapshot, output, config, position),
            |WorkspacesMessage::Clicked { id, is_active }| Message::WorkspaceClicked {
                id,
                is_active,
            },
        )],
        config,
        chrome,
    );
    let start = measured(start, chrome, window, Zone::BottomStart);
    let middle = container(measured(
        Row::with_children(
            slot(
                bar.window_group.view(
                    &bar.niri_snapshot,
                    &bar.desktop_entries,
                    bar.audio_group.window_audio(),
                    output,
                    config,
                    position,
                ),
                |m| match m {
                    WindowGroupMessage::WindowList(WindowListMessage::Tooltip(t)) => {
                        Message::Tooltip(t)
                    }
                    m => Message::WindowGroup(m),
                },
            )
            .map(|group| pill(group, config, chrome)),
        )
        .into(),
        chrome,
        window,
        Zone::BottomMiddle,
    ))
    .center_x(Length::Fill);
    let end = measured(
        Row::with_children(
            slot(bar.tray_group.view(config, position), |m| match m {
                TrayMessage::Tooltip(t) => Message::Tooltip(t),
                m => Message::Tray(m),
            })
            .map(|group| pill(group, config, chrome)),
        )
        .into(),
        chrome,
        window,
        Zone::BottomEnd,
    );
    row![start, middle, end]
        .align_y(Alignment::Center)
        .padding([0.0, config.spacing.group_padding_px])
        .into()
}

fn bar_row<'a>(
    start: Element<'a, Message>,
    center: Element<'a, Message>,
    end: Element<'a, Message>,
    config: &Config,
) -> Element<'a, Message> {
    row![
        start,
        Space::new().width(Length::Fill),
        center,
        Space::new().width(Length::Fill),
        end,
    ]
    .align_y(Alignment::Center)
    .padding([0.0, config.spacing.group_padding_px])
    .into()
}
