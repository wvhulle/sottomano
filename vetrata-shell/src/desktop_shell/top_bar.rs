use iced::Element;
use vetrata_config::screen_edges::Position;
use vetrata_widgets::{
    shell_message::Message, widget_groups::notification_group::NotificationStatus,
    widgets::widget_group::GroupContent,
};

use crate::desktop_shell::DesktopShell;

pub(super) struct TopBarGroups<'a> {
    pub start: [GroupContent<'a, Message>; 10],
    pub center: [GroupContent<'a, Message>; 3],
    pub end: [GroupContent<'a, Message>; 14],
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

    pub(super) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet: self.quiet_schedule_group.is_quiet(),
            live: self.notifications.live_count(),
            held: self.notifications.held_count(),
        }
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
                        self.shell_settings.notifications,
                        self.notification_status(),
                    )
                    .map(Message::NotificationGroup),
                self.quiet_schedule_group
                    .group_content(config)
                    .map(Message::QuietSchedule),
            ],
            end: [
                self.claude_usage_group
                    .group_content()
                    .map(Message::ClaudeUsage),
                self.copilot_usage_group
                    .group_content()
                    .map(Message::CopilotUsage),
                self.openrouter_credit_group
                    .group_content()
                    .map(Message::OpenRouterCredit),
                self.llama_server_group
                    .group_content()
                    .map(Message::LlamaServer),
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
