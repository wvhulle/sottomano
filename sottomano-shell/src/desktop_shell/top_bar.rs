use iced::Element;
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    widget_groups::notification_group::NotificationStatus, widgets::widget_group::GroupContent,
};

use crate::{
    desktop_shell::{DesktopShell, focused_app_id},
    shell_message::Message,
};

pub(super) struct TopBarGroups<'a> {
    pub start: Vec<GroupContent<'a, Message>>,
    pub center: [GroupContent<'a, Message>; 6],
    pub end: Vec<GroupContent<'a, Message>>,
}

impl DesktopShell {
    pub(super) fn top_bar<'a>(&'a self) -> Element<'a, Message> {
        let config = &self.config;
        let TopBarGroups { start, center, end } = self.top_bar_groups();
        let view = |group: GroupContent<'a, Message>| group.view(config, Position::Top, self.clock);
        self.panel_bar(
            Position::Top,
            start.into_iter().map(view),
            center.map(view),
            end.into_iter().map(view),
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
                self.power_group.group_content(config).map(Message::Power),
                self.compute_group.group_content().map(Message::Compute),
                self.memory_board_group
                    .group_content()
                    .map(Message::MemoryBoard),
            ]
            .into_iter()
            .chain(
                self.gpu_group
                    .group_contents()
                    .map(|group| group.map(Message::Gpu)),
            )
            .chain([
                self.storage_space_group
                    .group_content()
                    .map(Message::Storage),
                self.physical_drive_group
                    .group_content()
                    .map(Message::Drive),
                self.net_interfaces_group
                    .group_content(self.clock)
                    .map(Message::NetInterfaces),
            ])
            .chain(
                self.ethernet_ports_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(Message::EthernetPorts)),
            )
            .chain(
                self.bluetooth_adapters_group
                    .group_contents(
                        &|hardware_address| {
                            self.gamepad_group
                                .report_timing_for_address(hardware_address)
                        },
                        self.clock,
                    )
                    .into_iter()
                    .map(|group| group.map(Message::BluetoothAdapters)),
            )
            .chain([
                self.reachability_group
                    .group_content()
                    .map(Message::Reachability),
                self.vpn_group.group_content().map(Message::Vpn),
            ])
            .collect(),
            center: self.top_bar_center_groups(),
            end: self.top_bar_end_groups(),
        }
    }

    fn top_bar_center_groups(&self) -> [GroupContent<'_, Message>; 6] {
        let config = &self.config;
        [
            self.clock_group.group_content().map(Message::Clock),
            self.weather_group
                .group_content(config)
                .map(Message::Weather),
            self.caldav_group.group_content().map(Message::Caldav),
            self.notification_group
                .group_content(self.notification_status())
                .map(Message::NotificationGroup),
            self.quiet_schedule_group
                .group_content(config)
                .map(Message::QuietSchedule),
            self.session_focus_group
                .group_content(config)
                .map(Message::SessionFocus),
        ]
    }

    fn top_bar_end_groups(&self) -> Vec<GroupContent<'_, Message>> {
        let config = &self.config;
        let media_groups = self
            .media_group
            .group_contents(config, self.clock)
            .map(|group| group.map(Message::Media));
        let gamepad_groups = self
            .gamepad_group
            .group_contents(&config.widget_groups.gamepad, focused_app_id(&self.niri))
            .into_iter()
            .map(|group| group.map(Message::Gamepad));
        media_groups
            .chain([
                self.audio_group.group_content(config).map(Message::Audio),
                self.voice_dictation_group
                    .group_content()
                    .map(Message::VoiceDictation),
            ])
            .chain(gamepad_groups)
            .chain([self
                .session_menu
                .group_content(config)
                .map(Message::Session)])
            .collect()
    }
}
