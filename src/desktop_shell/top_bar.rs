use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, Space, column, container, mouse_area, row},
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    widgets::{
        region_reporter::{RegionRoles, region_reporter},
        widget_group::GroupEvent,
    },
};

impl DesktopShell {
    pub(super) fn top_bar(&self) -> Element<'_, Message> {
        let config = &self.config;
        let clusters = row![
            self.group_cluster(self.top_start_groups()),
            Space::new().width(Length::Fill),
            self.group_cluster(self.top_center_groups()),
            Space::new().width(Length::Fill),
            self.group_cluster(self.top_end_groups()),
        ]
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .padding([0.0, config.spacing.group_padding_px]);
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
        groups: impl IntoIterator<Item = Option<Element<'a, Message>>>,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let panels = &self.shell_settings.panels;
        let groups_disconnected = self.shell_settings.layout.disconnected_groups;
        let frame_style = self.shell_settings.frame_style(&config.palette);
        let framed = groups.into_iter().flatten().map(|group| {
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

    pub(super) fn top_start_groups(&self) -> [Option<Element<'_, Message>>; 9] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.weather_group
                .group_content()
                .map(Message::Weather)
                .view(config, top)
                .map(lift),
            self.battery_group
                .group_content(config)
                .map(Message::Battery)
                .view(config, top)
                .map(lift),
            self.compute_group
                .group_content()
                .map(Message::Compute)
                .view(config, top)
                .map(lift),
            self.disk_group
                .group_content()
                .map(Message::Disk)
                .view(config, top)
                .map(lift),
            self.net_interfaces_group
                .group_content()
                .map(Message::NetInterfaces)
                .view(config, top)
                .map(lift),
            self.systemd_manager_group
                .group_content(config)
                .map(Message::SystemdManager)
                .view(config, top)
                .map(lift),
            self.journal_events_group
                .group_content(config)
                .map(Message::JournalEvents)
                .view(config, top)
                .map(lift),
            self.pending_upgrades_group
                .group_content(config)
                .map(Message::PendingUpgrades)
                .view(config, top)
                .map(lift),
            self.internet_group
                .group_content()
                .map(Message::Internet)
                .view(config, top)
                .map(lift),
        ]
    }

    pub(super) fn top_center_groups(&self) -> [Option<Element<'_, Message>>; 2] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.clock_group
                .group_content()
                .map(Message::Clock)
                .view(config, top)
                .map(lift),
            self.notification_group
                .group_content(
                    config,
                    &self.shell_settings.notifications,
                    self.notifications.live_count(),
                )
                .map(Message::NotificationGroup)
                .view(config, top)
                .map(lift),
        ]
    }

    pub(super) fn top_end_groups(&self) -> [Option<Element<'_, Message>>; 8] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.ai_group
                .group_content()
                .map(Message::Ai)
                .view(config, top)
                .map(lift),
            self.monitor_group
                .group_content(config)
                .map(Message::Monitor)
                .view(config, top)
                .map(lift),
            self.dark_mode_group
                .group_content(config)
                .map(Message::DarkMode)
                .view(config, top)
                .map(lift),
            self.color_temperature_group
                .group_content(config)
                .map(Message::ColorTemperature)
                .view(config, top)
                .map(lift),
            self.idle_timeouts_group
                .group_content(config)
                .map(Message::IdleTimeouts)
                .view(config, top)
                .map(lift),
            self.wallpaper_group
                .group_content()
                .map(Message::Wallpaper)
                .view(config, top)
                .map(lift),
            self.media_group
                .group_content(config, &self.desktop_entries)
                .map(Message::Media)
                .view(config, top)
                .map(lift),
            self.audio_group
                .group_content(config)
                .map(Message::Audio)
                .view(config, top)
                .map(lift),
        ]
    }
}

pub(super) fn lift(element: Element<'_, GroupEvent<Message>>) -> Element<'_, Message> {
    element.map(Message::from)
}
