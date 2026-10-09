use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, Space, column, container, mouse_area, row},
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    widgets::{
        region_reporter::{RegionRoles, region_reporter},
        widget_group::GroupViewWithAlerts,
    },
};

impl DesktopShell {
    pub(super) fn top_bar(&self) -> Element<'_, Message> {
        let cluster = |groups: Vec<_>| self.widget_group_cluster(groups);
        let clusters = row![
            cluster(self.top_start_groups().into()),
            Space::new().width(Length::Fill),
            cluster(self.top_center_groups().into()),
            Space::new().width(Length::Fill),
            cluster(self.top_end_groups().into()),
        ]
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, self.config.spacing.group_padding_px]);
        self.bar_band(clusters)
    }

    pub(super) fn bar_band<'a>(
        &'a self,
        content: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let panels = &self.shell_settings.panels;
        let palette = &self.config.palette;
        let centered = mouse_area(
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_y(Vertical::Center),
        )
        .on_press(Message::DismissPressed);
        let groups_joined = !self.shell_settings.layout.disconnected_groups;
        let band_style = if groups_joined {
            self.shell_settings.frame_style(palette)
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
            corner_radius: panels.corner_radius.top_left,
        };
        region_reporter(column![strip], roles).into()
    }

    pub(super) fn group_cluster<'a>(
        &'a self,
        groups: Vec<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = &self.shell_settings.panels;
        let groups_disconnected = self.shell_settings.layout.disconnected_groups;
        let frame_style = self.shell_settings.frame_style(&config.palette);
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
                    panels.frame_corner_inset() + config.spacing.group_padding_px,
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
            corner_radius: panels.corner_radius.top_left,
        };
        region_reporter(cluster, roles).into()
    }

    pub(super) fn widget_group_cluster<'a>(
        &'a self,
        groups: Vec<GroupViewWithAlerts<'a, Message>>,
    ) -> Element<'a, Message> {
        let rendered = rendered_widget_groups(groups).collect();
        self.group_cluster(rendered)
    }

    pub(super) fn top_start_groups(&self) -> [GroupViewWithAlerts<'_, Message>; 8] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.weather_group
                .widget_group()
                .place_on_bar(Message::Weather, config, top),
            self.battery_group
                .widget_group(config)
                .place_on_bar(Message::Battery, config, top),
            self.compute_group
                .widget_group()
                .place_on_bar(Message::Compute, config, top),
            self.disk_group
                .widget_group()
                .place_on_bar(Message::Disk, config, top),
            self.net_interfaces_group.widget_group().place_on_bar(
                Message::NetInterfaces,
                config,
                top,
            ),
            self.systemd_units_group.widget_group(config).place_on_bar(
                Message::SystemdUnits,
                config,
                top,
            ),
            self.journal_events_group.widget_group(config).place_on_bar(
                Message::JournalEvents,
                config,
                top,
            ),
            self.internet_group
                .widget_group()
                .place_on_bar(Message::Internet, config, top),
        ]
    }

    pub(super) fn top_center_groups(&self) -> [GroupViewWithAlerts<'_, Message>; 2] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.clock_group
                .widget_group()
                .place_on_bar(Message::Clock, config, top),
            self.notification_group
                .widget_group(
                    config,
                    &self.shell_settings.notifications,
                    self.notifications.live_count(),
                )
                .place_on_bar(Message::NotificationGroup, config, top),
        ]
    }

    pub(super) fn top_end_groups(&self) -> [GroupViewWithAlerts<'_, Message>; 8] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.ai_group
                .widget_group()
                .place_on_bar(Message::Ai, config, top),
            self.monitor_group
                .widget_group(config)
                .place_on_bar(Message::Monitor, config, top),
            self.dark_mode_group
                .widget_group(config)
                .place_on_bar(Message::DarkMode, config, top),
            self.color_temperature_group
                .widget_group(config)
                .place_on_bar(Message::ColorTemperature, config, top),
            self.idle_timeouts_group.widget_group(config).place_on_bar(
                Message::IdleTimeouts,
                config,
                top,
            ),
            self.wallpaper_group
                .widget_group()
                .place_on_bar(Message::Wallpaper, config, top),
            self.media_group
                .widget_group(config, &self.desktop_entries)
                .place_on_bar(Message::Media, config, top),
            self.audio_group
                .widget_group(config)
                .place_on_bar(Message::Audio, config, top),
        ]
    }
}

pub(super) fn rendered_widget_groups<'a>(
    groups: impl IntoIterator<Item = GroupViewWithAlerts<'a, Message>>,
) -> impl Iterator<Item = Element<'a, Message>> {
    groups
        .into_iter()
        .filter_map(|group| group.element)
        .map(|element| element.map(Message::from))
}
