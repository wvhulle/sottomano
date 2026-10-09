use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Row, Space, column, container, mouse_area, row},
};

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    widgets::{
        region_reporter::{RegionRoles, region_reporter},
        widget_group::WidgetGroup,
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
        groups: Vec<WidgetGroup<'a, Message>>,
    ) -> Element<'a, Message> {
        let rendered = rendered_widget_groups(groups).collect();
        self.group_cluster(rendered)
    }

    pub(super) fn top_start_groups(&self) -> [WidgetGroup<'_, Message>; 9] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.weather_group.widget_group(config, top, Message::Weather),
            self.battery_group.widget_group(config, top, Message::Battery),
            self.compute_group.widget_group(config, top, Message::Compute),
            self.disk_group.widget_group(config, top, Message::Disk),
            self.net_interfaces_group.widget_group(config, top, Message::NetInterfaces),
            self.systemd_manager_group.widget_group(config, top, Message::SystemdManager),
            self.journal_events_group.widget_group(config, top, Message::JournalEvents),
            self.pending_upgrades_group.widget_group(config, top, Message::PendingUpgrades),
            self.internet_group.widget_group(config, top, Message::Internet),
        ]
    }

    pub(super) fn top_center_groups(&self) -> [WidgetGroup<'_, Message>; 2] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.clock_group.widget_group(config, top, Message::Clock),
            self.notification_group
                .widget_group(
                    config,
                    &self.shell_settings.notifications,
                    self.notifications.live_count(),
                    top,
                    Message::NotificationGroup,
                ),
        ]
    }

    pub(super) fn top_end_groups(&self) -> [WidgetGroup<'_, Message>; 8] {
        let config = &self.config;
        let top = Position::Top;
        [
            self.ai_group.widget_group(config, top, Message::Ai),
            self.monitor_group.widget_group(config, top, Message::Monitor),
            self.dark_mode_group.widget_group(config, top, Message::DarkMode),
            self.color_temperature_group.widget_group(config, top, Message::ColorTemperature),
            self.idle_timeouts_group.widget_group(config, top, Message::IdleTimeouts),
            self.wallpaper_group.widget_group(config, top, Message::Wallpaper),
            self.media_group
                .widget_group(config, &self.desktop_entries, top, Message::Media),
            self.audio_group.widget_group(config, top, Message::Audio),
        ]
    }
}

pub(super) fn rendered_widget_groups<'a>(
    groups: impl IntoIterator<Item = WidgetGroup<'a, Message>>,
) -> impl Iterator<Item = Element<'a, Message>> {
    groups
        .into_iter()
        .filter_map(|group| group.element)
        .map(|element| element.map(Message::from))
}
