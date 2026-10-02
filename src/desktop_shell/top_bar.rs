use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, Space, column, container, mouse_area, row},
    window::Id,
};

use crate::{
    desktop_shell::{
        DesktopShell, Message,
        monitor_surfaces::{Position, ShellElement},
    },
    widgets::{
        region_reporter::{RegionRoles, region_reporter},
        widget_group::WidgetGroup,
        widget_group_frame::widget_group_frame,
    },
};

pub struct TopBar<'a> {
    shell: &'a DesktopShell,
    window: Id,
}

impl<'a> TopBar<'a> {
    pub fn new(shell: &'a DesktopShell, window: Id) -> Self {
        Self { shell, window }
    }
}

impl<'a> From<TopBar<'a>> for Element<'a, Message> {
    fn from(TopBar { shell, window }: TopBar<'a>) -> Self {
        let spacing = &shell.config.spacing;
        let cluster = |element, groups| {
            GroupCluster::of_widget_groups(shell, window, element, groups, Position::Top)
        };
        let clusters = row![
            cluster(ShellElement::TopBarStart, top_start_groups(shell).into()),
            Space::new().width(Length::Fill),
            cluster(ShellElement::TopBarCenter, top_center_groups(shell).into()),
            Space::new().width(Length::Fill),
            cluster(ShellElement::TopBarEnd, top_end_groups(shell).into()),
        ]
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, spacing.group_padding_px]);
        BarBand::new(shell, window, ShellElement::TopBar, clusters).into()
    }
}

pub struct BarBand<'a> {
    shell: &'a DesktopShell,
    window: Id,
    element: ShellElement,
    content: Element<'a, Message>,
}

impl<'a> BarBand<'a> {
    pub fn new(
        shell: &'a DesktopShell,
        window: Id,
        element: ShellElement,
        content: impl Into<Element<'a, Message>>,
    ) -> Self {
        Self {
            shell,
            window,
            element,
            content: content.into(),
        }
    }
}

impl<'a> From<BarBand<'a>> for Element<'a, Message> {
    fn from(band: BarBand<'a>) -> Self {
        let BarBand {
            shell,
            window,
            element,
            content,
        } = band;
        let layout = &shell.panel_settings.layout;
        let palette = &shell.config.palette;
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !layout.disconnected_groups;
        let strip = if groups_joined {
            widget_group_frame(centered, layout, palette)
        } else {
            let foreground = palette.bar_foreground;
            container(centered).style(move |_| container::Style {
                text_color: Some(foreground),
                ..container::Style::default()
            })
        }
        .width(Length::Fill)
        .height(Length::Fixed(shell.config.bar.height_px));
        let roles = RegionRoles {
            takes_input: true,
            blurred: groups_joined,
        };
        region_reporter(column![strip], roles, move |region| {
            Message::ElementRegions {
                window,
                element,
                region,
            }
        })
        .into()
    }
}

pub struct GroupCluster<'a> {
    shell: &'a DesktopShell,
    window: Id,
    element: ShellElement,
    groups: Vec<Element<'a, Message>>,
}

impl<'a> GroupCluster<'a> {
    pub fn new(
        shell: &'a DesktopShell,
        window: Id,
        element: ShellElement,
        groups: Vec<Element<'a, Message>>,
    ) -> Self {
        Self {
            shell,
            window,
            element,
            groups,
        }
    }

    pub fn of_widget_groups(
        shell: &'a DesktopShell,
        window: Id,
        element: ShellElement,
        groups: Vec<WidgetGroup<'a, Message>>,
        position: Position,
    ) -> Self {
        let rendered = groups
            .into_iter()
            .filter_map(|group| group.view(&shell.config, position))
            .map(|element| element.map(Message::from))
            .collect();
        Self::new(shell, window, element, rendered)
    }
}

impl<'a> From<GroupCluster<'a>> for Element<'a, Message> {
    fn from(cluster: GroupCluster<'a>) -> Self {
        let GroupCluster {
            shell,
            window,
            element,
            groups,
        } = cluster;
        let config = &shell.config;
        let layout = &shell.panel_settings.layout;
        let groups_disconnected = layout.disconnected_groups;
        let framed = groups.into_iter().map(|group| {
            if !groups_disconnected {
                return group;
            }
            widget_group_frame(group, layout, &config.palette)
                .height(Length::Fixed(config.bar.height_px))
                .align_y(Vertical::Center)
                .padding([
                    0.0,
                    layout.frame_corner_inset() + config.spacing.group_padding_px,
                ])
                .into()
        });
        let spacing = if groups_disconnected {
            config.spacing.widget_spacing_px
        } else {
            config.spacing.group_spacing_px
        };
        let cluster = Row::with_children(framed)
            .spacing(spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center);
        let roles = RegionRoles {
            takes_input: false,
            blurred: groups_disconnected,
        };
        region_reporter(cluster, roles, move |region| Message::ElementRegions {
            window,
            element,
            region,
        })
        .into()
    }
}

pub(super) fn top_start_groups(shell: &DesktopShell) -> [WidgetGroup<'_, Message>; 8] {
    let config = &shell.config;
    [
        shell
            .weather_group
            .widget_group()
            .map_messages(Message::Weather),
        shell
            .battery_group
            .widget_group(config)
            .map_messages(Message::Battery),
        shell
            .system_group
            .widget_group()
            .map_messages(Message::System),
        shell
            .net_interfaces_group
            .widget_group()
            .map_messages(Message::NetInterfaces),
        shell
            .journal_group
            .widget_group(config)
            .map_messages(Message::Journal),
        shell
            .internet_group
            .widget_group()
            .map_messages(Message::Internet),
        shell
            .panel_settings_group
            .widget_group(config, &shell.panel_settings.layout)
            .map_messages(Message::PanelSettings),
        shell
            .niri_group
            .widget_group(config)
            .map_messages(Message::NiriStyling),
    ]
}

pub(super) fn top_center_groups(shell: &DesktopShell) -> [WidgetGroup<'_, Message>; 2] {
    let config = &shell.config;
    [
        shell
            .clock_group
            .widget_group(config)
            .map_messages(Message::Clock),
        shell
            .notification_group
            .widget_group(
                config,
                &shell.panel_settings.toasts,
                shell.notifications.live_count(),
            )
            .map_messages(Message::NotificationGroup),
    ]
}

pub(super) fn top_end_groups(shell: &DesktopShell) -> [WidgetGroup<'_, Message>; 8] {
    let config = &shell.config;
    [
        shell.ai_group.widget_group().map_messages(Message::Ai),
        shell
            .monitor_group
            .widget_group(config)
            .map_messages(Message::Monitor),
        shell
            .dark_mode_group
            .widget_group(config)
            .map_messages(Message::DarkMode),
        shell
            .color_temperature_group
            .widget_group(config)
            .map_messages(Message::ColorTemperature),
        shell
            .idle_timeouts_group
            .widget_group(config)
            .map_messages(Message::IdleTimeouts),
        shell
            .wallpaper_group
            .widget_group()
            .map_messages(Message::Wallpaper),
        shell
            .media_group
            .widget_group(config, &shell.desktop_entries)
            .map_messages(Message::Media),
        shell
            .audio_group
            .widget_group(config)
            .map_messages(Message::Audio),
    ]
}
