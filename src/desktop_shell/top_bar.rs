use iced::Element;

use crate::{
    desktop_shell::{DesktopShell, Message, monitor_surfaces::Position},
    widgets::widget_group::GroupContent,
};

pub(super) struct TopBarGroups<'a> {
    pub start: [GroupContent<'a, Message>; 10],
    pub center: [GroupContent<'a, Message>; 3],
    pub end: [GroupContent<'a, Message>; 11],
}

impl DesktopShell {
    pub(super) fn top_bar<'a>(&'a self) -> Element<'a, Message> {
        let config = &self.config;
        let TopBarGroups { start, center, end } = self.top_bar_groups();
        let view = |group: GroupContent<'a, Message>| group.view(config, Position::Top);
        self.panel_bar(
            Position::Top,
            start.map(view),
            center.map(view),
            end.map(view),
        )
    }

    pub(super) fn top_bar_groups(&self) -> TopBarGroups<'_> {
        let config = &self.config;
        TopBarGroups {
            start: [
                self.weather_group.group_content().map(Message::Weather),
                self.power_group.group_content(config).map(Message::Power),
                self.compute_group.group_content().map(Message::Compute),
                self.gpu_group.group_content().map(Message::Gpu),
                self.disk_group.group_content().map(Message::Disk),
                self.net_interfaces_group
                    .group_content()
                    .map(Message::NetInterfaces),
                self.systemd_manager_group
                    .group_content(config)
                    .map(Message::SystemdManager),
                self.journal_events_group
                    .group_content(config)
                    .map(Message::JournalEvents),
                self.pending_upgrades_group
                    .group_content(config)
                    .map(Message::PendingUpgrades),
                self.internet_group.group_content().map(Message::Internet),
            ],
            center: [
                self.clock_group.group_content().map(Message::Clock),
                self.notification_group
                    .group_content(
                        config,
                        &self.shell_settings.notifications,
                        self.notifications.live_count(),
                        self.notifications.held_count(),
                        self.quiet_schedule_group.is_quiet(),
                    )
                    .map(Message::NotificationGroup),
                self.quiet_schedule_group
                    .group_content(config)
                    .map(Message::QuietSchedule),
            ],
            end: [
                self.ai_group.group_content().map(Message::Ai),
                self.monitor_group
                    .group_content(config)
                    .map(Message::Monitor),
                self.dark_mode_group
                    .group_content(config)
                    .map(Message::DarkMode),
                self.color_temperature_group
                    .group_content(config)
                    .map(Message::ColorTemperature),
                self.idle_timeouts_group
                    .group_content(config)
                    .map(Message::IdleTimeouts),
                self.wallpaper_group.group_content().map(Message::Wallpaper),
                self.media_group
                    .group_content(config, &self.desktop_entries)
                    .map(Message::Media),
                self.audio_group.group_content(config).map(Message::Audio),
                self.session_focus_group
                    .group_content(config, self.quiet_schedule_group.waking_window())
                    .map(Message::SessionFocus),
                self.login_session_group
                    .group_content()
                    .map(Message::LoginSession),
                self.session_menu
                    .group_content(config)
                    .map(Message::Session),
            ],
        }
    }
}
