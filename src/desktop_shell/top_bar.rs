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
    },
};

impl DesktopShell {
    pub(super) fn top_bar(&self, window: Id) -> Element<'_, Message> {
        let cluster = |element, groups: Vec<_>| {
            self.widget_group_cluster(window, element, groups, Position::Top)
        };
        let clusters = row![
            cluster(ShellElement::TopBarStart, self.top_start_groups().into()),
            Space::new().width(Length::Fill),
            cluster(ShellElement::TopBarCenter, self.top_center_groups().into()),
            Space::new().width(Length::Fill),
            cluster(ShellElement::TopBarEnd, self.top_end_groups().into()),
        ]
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, self.config.spacing.group_padding_px]);
        self.bar_band(window, ShellElement::TopBar, clusters)
    }

    pub(super) fn bar_band<'a>(
        &'a self,
        window: Id,
        element: ShellElement,
        content: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let layout = &self.panel_settings.layout;
        let palette = &self.config.palette;
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !layout.disconnected_groups;
        let band_style = if groups_joined {
            layout.frame_style(palette)
        } else {
            container::Style::default().color(palette.bar_foreground)
        };
        let strip = container(centered)
            .style(move |_| band_style)
            .width(Length::Fill)
            .height(Length::Fixed(self.config.bar.height_px));
        let roles = RegionRoles {
            takes_input: true,
            blurred: groups_joined,
        };
        region_reporter(column![strip], roles, move |region| Message::ElementRegions {
            window,
            element,
            region,
        })
        .into()
    }

    pub(super) fn group_cluster<'a>(
        &'a self,
        window: Id,
        element: ShellElement,
        groups: Vec<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let layout = &self.panel_settings.layout;
        let groups_disconnected = layout.disconnected_groups;
        let frame_style = layout.frame_style(&config.palette);
        let framed = groups.into_iter().map(|group| {
            if !groups_disconnected {
                return group;
            }
            container(group)
                .style(move |_| frame_style)
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

    pub(super) fn widget_group_cluster<'a>(
        &'a self,
        window: Id,
        element: ShellElement,
        groups: Vec<WidgetGroup<'a, Message>>,
        position: Position,
    ) -> Element<'a, Message> {
        let rendered = groups
            .into_iter()
            .filter_map(|group| group.view(&self.config, position))
            .map(|element| element.map(Message::from))
            .collect();
        self.group_cluster(window, element, rendered)
    }

    pub(super) fn top_start_groups(&self) -> [WidgetGroup<'_, Message>; 8] {
        let config = &self.config;
        [
            self.weather_group.widget_group().map_messages(Message::Weather),
            self.battery_group
                .widget_group(config)
                .map_messages(Message::Battery),
            self.system_group.widget_group().map_messages(Message::System),
            self.net_interfaces_group
                .widget_group()
                .map_messages(Message::NetInterfaces),
            self.journal_group
                .widget_group(config)
                .map_messages(Message::Journal),
            self.internet_group
                .widget_group()
                .map_messages(Message::Internet),
            self.panel_settings_group
                .widget_group(config, &self.panel_settings.layout)
                .map_messages(Message::PanelSettings),
            self.niri_group
                .widget_group(config)
                .map_messages(Message::NiriStyling),
        ]
    }

    pub(super) fn top_center_groups(&self) -> [WidgetGroup<'_, Message>; 2] {
        let config = &self.config;
        [
            self.clock_group
                .widget_group(config)
                .map_messages(Message::Clock),
            self.notification_group
                .widget_group(
                    config,
                    &self.panel_settings.toasts,
                    self.notifications.live_count(),
                )
                .map_messages(Message::NotificationGroup),
        ]
    }

    pub(super) fn top_end_groups(&self) -> [WidgetGroup<'_, Message>; 8] {
        let config = &self.config;
        [
            self.ai_group.widget_group().map_messages(Message::Ai),
            self.monitor_group
                .widget_group(config)
                .map_messages(Message::Monitor),
            self.dark_mode_group
                .widget_group(config)
                .map_messages(Message::DarkMode),
            self.color_temperature_group
                .widget_group(config)
                .map_messages(Message::ColorTemperature),
            self.idle_timeouts_group
                .widget_group(config)
                .map_messages(Message::IdleTimeouts),
            self.wallpaper_group
                .widget_group()
                .map_messages(Message::Wallpaper),
            self.media_group
                .widget_group(config, &self.desktop_entries)
                .map_messages(Message::Media),
            self.audio_group
                .widget_group(config)
                .map_messages(Message::Audio),
        ]
    }
}
