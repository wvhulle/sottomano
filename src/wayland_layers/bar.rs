use core::f32::consts::FRAC_1_SQRT_2;

use iced::{
    Alignment, Element, Length, Vector,
    alignment::Vertical,
    border::Radius,
    widget::{Row, Space, container, mouse_area, row, stack, text},
    window::Id,
};
use iced_exwlshell::reexport::{KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption};

use crate::{
    app::{Bar, Message},
    config::Config,
    state::{PanelLayout, PanelSettings, WorkspacesPlacement},
    wayland_layers::{
        backdrop_shadow,
        blur_caster::{self, Zone},
        sidebar,
        surface::{Margin, Position},
        tooltip,
    },
    widget_groups::{
        output_group::{self, OutputMessage},
        workspaces_group,
    },
    widgets::{
        bounds_probe::bounds_probe,
        widget_group::{GroupEvent, WidgetGroup},
        window_icon::WindowAssets,
    },
};

pub fn view(bar: &Bar, window: Id) -> Element<'_, Message> {
    if bar.backdrops.contains_key(&window) {
        return backdrop_shadow::fill();
    }
    if bar.blur_casters.contains(window) {
        return blur_caster::fill();
    }
    let Some(surface) = bar.surfaces.get(&window) else {
        return container(text("")).into();
    };
    match surface.position {
        Position::Top => dismissable_strip(
            bar,
            tooltip::on_surface(window, Vector::ZERO, top_row(bar, window, surface.position)),
        ),
        Position::Bottom => dismissable_strip(
            bar,
            tooltip::on_surface(
                window,
                Vector::ZERO,
                bottom_row(bar, window, &surface.output, surface.position),
            ),
        ),
        Position::Left => tooltip::on_surface(
            window,
            sidebar::origin_in_tooltip_overlay(&bar.panel_settings.layout),
            sidebar::view(bar, window, &surface.output),
        ),
    }
}

pub fn layer_settings(
    output: &str,
    position: Position,
    height: u32,
    margin: Margin,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: position.strip_size(height),
        exclusive_zone: Some(height.cast_signed()),
        anchor: position.anchor(),
        layer: Layer::Top,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(margin.edges()),
        ..Default::default()
    }
}

fn dismissable_strip<'a>(bar: &'a Bar, content: Element<'a, Message>) -> Element<'a, Message> {
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
            background: (!bar.panel_settings.layout.disconnected_groups).then(|| {
                bar.config
                    .palette
                    .surface
                    .scale_alpha(bar.panel_settings.layout.tint_factor)
                    .into()
            }),
            border: iced::Border {
                radius: bar.panel_settings.layout.corner_radius,
                ..Default::default()
            },
            text_color: Some(bar.config.palette.bar_foreground),
            ..Default::default()
        })
        .into()
}

fn top_start_groups(bar: &Bar) -> [WidgetGroup<'_, Message>; 7] {
    let config = &bar.config;
    [
        bar.weather_group
            .widget_group()
            .map_messages(Message::Weather),
        bar.battery_group
            .widget_group(config)
            .map_messages(Message::Battery),
        bar.system_group
            .widget_group()
            .map_messages(Message::System),
        bar.net_interfaces_group
            .widget_group()
            .map_messages(Message::NetInterfaces),
        bar.journal_group
            .widget_group(config)
            .map_messages(Message::Journal),
        bar.internet_group
            .widget_group()
            .map_messages(Message::Internet),
        bar.panel_settings_group
            .widget_group(config, &bar.panel_settings.layout)
            .map_messages(Message::PanelSettings),
    ]
}

fn top_center_groups(bar: &Bar) -> [WidgetGroup<'_, Message>; 2] {
    let config = &bar.config;
    [
        bar.clock_group
            .widget_group(config)
            .map_messages(Message::Clock),
        bar.notification_group
            .widget_group(
                config,
                &bar.panel_settings.toasts,
                bar.notifications.live_count(),
            )
            .map_messages(Message::NotificationGroup),
    ]
}

fn top_end_groups(bar: &Bar) -> [WidgetGroup<'_, Message>; 8] {
    let config = &bar.config;
    [
        bar.ai_group.widget_group().map_messages(Message::Ai),
        bar.monitor_group
            .widget_group(config)
            .map_messages(Message::Monitor),
        bar.dark_mode_group
            .widget_group(config)
            .map_messages(Message::DarkMode),
        bar.color_temperature_group
            .widget_group(config)
            .map_messages(Message::ColorTemperature),
        bar.idle_timeouts_group
            .widget_group(config)
            .map_messages(Message::IdleTimeouts),
        bar.wallpaper_group
            .widget_group()
            .map_messages(Message::Wallpaper),
        bar.media_group
            .widget_group(config, &bar.desktop_entries)
            .map_messages(Message::Media),
        bar.audio_group
            .widget_group(config)
            .map_messages(Message::Audio),
    ]
}

fn bottom_end_groups(bar: &Bar) -> [WidgetGroup<'_, Message>; 1] {
    [bar.tray_group
        .widget_group(&bar.config)
        .map_messages(Message::Tray)]
}

pub fn widget_groups(bar: &Bar) -> impl Iterator<Item = WidgetGroup<'_, Message>> {
    top_start_groups(bar)
        .into_iter()
        .chain(top_center_groups(bar))
        .chain(top_end_groups(bar))
        .chain(bottom_end_groups(bar))
}

fn routed(event: GroupEvent<Message>) -> Message {
    match event {
        GroupEvent::Group(message) => message,
        GroupEvent::Tooltip(tooltip) => Message::Tooltip(tooltip),
    }
}

fn top_row(bar: &Bar, window: Id, position: Position) -> Element<'_, Message> {
    let config = &bar.config;
    let panel_settings = &bar.panel_settings;
    let layout = &panel_settings.layout;
    let start = measured(
        zone(
            rendered(top_start_groups(bar), config, position),
            config,
            panel_settings,
        ),
        layout,
        window,
        Zone::TopStart,
    );
    let center = measured(
        zone(
            rendered(top_center_groups(bar), config, position),
            config,
            panel_settings,
        ),
        layout,
        window,
        Zone::TopCenter,
    );
    let end = measured(
        zone(
            rendered(top_end_groups(bar), config, position),
            config,
            panel_settings,
        ),
        layout,
        window,
        Zone::TopEnd,
    );
    bar_row(start, center, end, config)
}

fn rendered<'a>(
    groups: impl IntoIterator<Item = WidgetGroup<'a, Message>>,
    config: &'a Config,
    position: Position,
) -> impl Iterator<Item = Option<Element<'a, Message>>> {
    groups.into_iter().map(move |group| {
        group
            .view(config, position)
            .map(|element| element.map(routed))
    })
}

fn measured<'a>(
    content: Element<'a, Message>,
    layout: &PanelLayout,
    window: Id,
    zone: Zone,
) -> Element<'a, Message> {
    if !layout.disconnected_groups {
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
    panel_settings: &PanelSettings,
) -> Element<'a, Message> {
    let spacing = if panel_settings.layout.disconnected_groups {
        config.spacing.widget_spacing_px
    } else {
        config.spacing.group_spacing_px
    };
    Row::with_children(
        groups
            .into_iter()
            .flatten()
            .map(|group| pill(group, config, panel_settings)),
    )
    .spacing(spacing)
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .into()
}

pub fn corner_inset(radius: Radius) -> f32 {
    radius.top_left * (1.0 - FRAC_1_SQRT_2)
}

fn pill<'a>(
    group: Element<'a, Message>,
    config: &Config,
    panel_settings: &PanelSettings,
) -> Element<'a, Message> {
    if !panel_settings.layout.disconnected_groups {
        return group;
    }
    let background = config
        .palette
        .surface
        .scale_alpha(panel_settings.layout.tint_factor);
    let radius = panel_settings.layout.corner_radius;
    container(group)
        .height(Length::Fixed(config.bar.height_px))
        .align_y(Vertical::Center)
        .padding([0.0, corner_inset(radius) + config.spacing.group_padding_px])
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius,
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

fn places<'a>(bar: &'a Bar, output: &str, position: Position) -> Element<'a, Message> {
    let config = &bar.config;
    let display = output_group::view(output, config, position).map(|m| match m {
        OutputMessage::Clicked => Message::OverviewToggled,
        OutputMessage::Tooltip(t) => Message::Tooltip(t),
    });
    let workspaces = match bar.panel_settings.layout.workspaces_placement {
        WorkspacesPlacement::Sidebar => None,
        WorkspacesPlacement::BottomPanel => {
            workspaces_group::inline_view(&bar.niri_snapshot, output, config, position)
                .map(|workspaces| workspaces.map(Message::from))
        }
    };
    Row::with_children([Some(display), workspaces].into_iter().flatten())
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

fn bottom_row<'a>(
    bar: &'a Bar,
    window: Id,
    output: &'a str,
    position: Position,
) -> Element<'a, Message> {
    let config = &bar.config;
    let panel_settings = &bar.panel_settings;
    let layout = &panel_settings.layout;
    let start = zone(
        [Some(places(bar, output, position))],
        config,
        panel_settings,
    );
    let start = measured(start, layout, window, Zone::BottomStart);
    let middle = container(measured(
        Row::with_children(
            slot(
                bar.window_group.view(
                    &bar.niri_snapshot,
                    WindowAssets {
                        entries: &bar.desktop_entries,
                        favicons: &bar.favicons,
                        fallbacks: &bar.font_fallbacks,
                    },
                    bar.audio_group.window_audio(),
                    output,
                    config,
                    position,
                ),
                |event| routed(event.map(Message::WindowGroup)),
            )
            .map(|group| pill(group, config, panel_settings)),
        )
        .into(),
        layout,
        window,
        Zone::BottomMiddle,
    ))
    .center_x(Length::Fill);
    let end = measured(
        Row::with_children(
            rendered(bottom_end_groups(bar), config, position)
                .flatten()
                .map(|group| pill(group, config, panel_settings)),
        )
        .into(),
        layout,
        window,
        Zone::BottomEnd,
    );
    row![start, middle, end]
        .spacing(config.spacing.group_spacing_px)
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
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .padding([0.0, config.spacing.group_padding_px])
    .into()
}
