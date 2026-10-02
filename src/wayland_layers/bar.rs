use core::f32::consts::FRAC_1_SQRT_2;

use iced::{
    Alignment, Color, Element, Length, Shadow, Vector,
    alignment::Vertical,
    border::Radius,
    widget::{Row, Space, column, container, mouse_area, row, text},
    window::Id,
};

use crate::{
    app::{Bar, Message},
    config::Config,
    state::{PanelLayout, PanelSettings, WorkspacesPlacement},
    wayland_layers::{
        backdrop_shadow,
        panel_regions::Zone,
        sidebar,
        surface::{self, Position, SurfaceRole},
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
    let Some(surface) = bar.surfaces.get(&window) else {
        return container(text("")).into();
    };
    match surface.role {
        SurfaceRole::Panel => panel(bar, window, &surface.output),
        SurfaceRole::Reservation(_) => surface::blank(),
    }
}

fn panel<'a>(bar: &'a Bar, window: Id, output: &'a str) -> Element<'a, Message> {
    let float = bar.panel_settings.layout.float_offset_px;
    let origin = Vector::new(-float, -float);
    let edge = |position, content| tooltip::on_surface(window, position, origin, content);
    let top = edge(
        Position::Top,
        band(
            bar,
            window,
            Zone::TopBar,
            top_row(bar, window, output, Position::Top),
        ),
    );
    let bottom = edge(
        Position::Bottom,
        band(
            bar,
            window,
            Zone::BottomBar,
            bottom_row(bar, window, output, Position::Bottom),
        ),
    );
    let side = container(edge(Position::Left, sidebar::view(bar, window, output)))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
    column![top, side, bottom].padding(float).into()
}

fn band<'a>(
    bar: &'a Bar,
    window: Id,
    zone: Zone,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let layout = &bar.panel_settings.layout;
    let centered = mouse_area(
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .on_press(Message::DismissPressed);
    let joined = !layout.disconnected_groups;
    let background = chrome_tint(&bar.config, layout);
    let shadow = pill_shadow(layout);
    let strip = container(centered)
        .width(Length::Fill)
        .height(Length::Fixed(bar.config.bar.height_px))
        .style(move |_| container::Style {
            background: joined.then(|| background.into()),
            border: iced::Border {
                radius: layout.corner_radius,
                ..Default::default()
            },
            shadow: if joined { shadow } else { Shadow::default() },
            text_color: Some(bar.config.palette.bar_foreground),
            ..Default::default()
        });
    bounds_probe(column![strip], move |rects| Message::PillBounds {
        window,
        zone,
        rects,
    })
}

fn chrome_tint(config: &Config, layout: &PanelLayout) -> Color {
    config.palette.surface.scale_alpha(layout.tint_factor)
}

fn top_start_groups(bar: &Bar) -> [WidgetGroup<'_, Message>; 8] {
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
        bar.niri_group
            .widget_group(config)
            .map_messages(Message::NiriStyling),
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

fn zone_pills<'a>(
    bar: &'a Bar,
    output: &'a str,
    position: Position,
    zone: Zone,
) -> Vec<Element<'a, Message>> {
    let config = &bar.config;
    let groups: Vec<Option<Element<'a, Message>>> = match zone {
        Zone::TopStart => rendered(top_start_groups(bar), config, position).collect(),
        Zone::TopCenter => rendered(top_center_groups(bar), config, position).collect(),
        Zone::TopEnd => rendered(top_end_groups(bar), config, position).collect(),
        Zone::BottomStart => vec![Some(places(bar, output, position))],
        Zone::BottomMiddle => vec![window_list(bar, output, position)],
        Zone::BottomEnd => rendered(bottom_end_groups(bar), config, position).collect(),
        Zone::TopBar | Zone::BottomBar | Zone::Sidebar => Vec::new(),
    };
    groups
        .into_iter()
        .flatten()
        .map(|group| pill(group, config, &bar.panel_settings))
        .collect()
}

fn top_row<'a>(
    bar: &'a Bar,
    window: Id,
    output: &'a str,
    position: Position,
) -> Element<'a, Message> {
    let config = &bar.config;
    let layout = &bar.panel_settings.layout;
    let [start, center, end] = [Zone::TopStart, Zone::TopCenter, Zone::TopEnd].map(|zone| {
        measured(
            zone_row(zone_pills(bar, output, position, zone), config, layout),
            layout,
            window,
            zone,
        )
    });
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

fn zone_row<'a>(
    pills: Vec<Element<'a, Message>>,
    config: &Config,
    layout: &PanelLayout,
) -> Element<'a, Message> {
    let spacing = if layout.disconnected_groups {
        config.spacing.widget_spacing_px
    } else {
        config.spacing.group_spacing_px
    };
    Row::with_children(pills)
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
    let layout = &panel_settings.layout;
    let background = chrome_tint(config, layout);
    let radius = layout.corner_radius;
    let shadow = pill_shadow(layout);
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
            shadow,
            ..Default::default()
        })
        .into()
}

pub fn pill_shadow(layout: &PanelLayout) -> Shadow {
    let style = layout.shadow;
    if !style.cast {
        return Shadow::default();
    }
    Shadow {
        color: Color::BLACK.scale_alpha(style.opacity),
        offset: Vector::ZERO,
        blur_radius: style.softness_px,
    }
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

fn window_list<'a>(
    bar: &'a Bar,
    output: &'a str,
    position: Position,
) -> Option<Element<'a, Message>> {
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
            &bar.config,
            position,
        ),
        |event| routed(event.map(Message::WindowGroup)),
    )
}

fn bottom_row<'a>(
    bar: &'a Bar,
    window: Id,
    output: &'a str,
    position: Position,
) -> Element<'a, Message> {
    let config = &bar.config;
    let layout = &bar.panel_settings.layout;
    let start = measured(
        zone_row(
            zone_pills(bar, output, position, Zone::BottomStart),
            config,
            layout,
        ),
        layout,
        window,
        Zone::BottomStart,
    );
    let middle = container(measured(
        Row::with_children(zone_pills(bar, output, position, Zone::BottomMiddle)).into(),
        layout,
        window,
        Zone::BottomMiddle,
    ))
    .center_x(Length::Fill);
    let end = measured(
        Row::with_children(zone_pills(bar, output, position, Zone::BottomEnd)).into(),
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
