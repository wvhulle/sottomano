use iced::Element;
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    widget_groups::{group_menus::GroupMenuKind, notification_group::NotificationStatus},
    widgets::{
        group_menu::GroupMenu,
        phosphor::{Phosphor, Weight},
        widget_group::GroupContent,
    },
};

use crate::{
    desktop_shell::{DesktopShell, focused_app_id},
    shell_message::Message,
};

pub(super) struct TopBarGroups<'a> {
    pub start_menus: [GroupMenu<'a, Message>; 2],
    pub center: [GroupContent<'a, Message>; 2],
    pub agenda_menu: GroupMenu<'a, Message>,
    pub notifications: GroupContent<'a, Message>,
    pub media_menu: GroupMenu<'a, Message>,
    pub end: [GroupContent<'a, Message>; 1],
}

impl DesktopShell {
    pub(super) fn top_bar<'a>(&'a self) -> Element<'a, Message> {
        let config = &self.config;
        let TopBarGroups {
            start_menus,
            center,
            agenda_menu,
            notifications,
            media_menu,
            end,
        } = self.top_bar_groups();
        let view = |group: GroupContent<'a, Message>| group.view(config, Position::Top, self.clock);
        self.panel_bar(
            Position::Top,
            start_menus
                .into_iter()
                .flat_map(|group_menu| group_menu.bar_views(config, Position::Top, self.clock)),
            center
                .map(view)
                .into_iter()
                .chain(agenda_menu.bar_views(config, Position::Top, self.clock))
                .chain([view(notifications)]),
            media_menu
                .bar_views(config, Position::Top, self.clock)
                .chain(end.map(view)),
        )
    }

    pub(super) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet: self.widget_groups.quiet_schedule_group.is_quiet(),
            live: self.notifications.live_count(),
            held: self.notifications.held_count(),
        }
    }

    pub(super) fn top_bar_groups(&self) -> TopBarGroups<'_> {
        let config = &self.config;
        TopBarGroups {
            start_menus: [self.hardware_menu(), self.network_menu()],
            center: [
                self.widget_groups
                    .clock_group
                    .group_content()
                    .map(Message::Clock),
                self.widget_groups
                    .weather_group
                    .group_content(config)
                    .map(Message::Weather),
            ],
            agenda_menu: self.agenda_menu(),
            notifications: self
                .widget_groups
                .notification_group
                .group_content(self.notification_status())
                .map(Message::NotificationGroup),
            media_menu: self.media_menu(),
            end: [self
                .widget_groups
                .session_menu
                .group_content(config)
                .map(Message::Session)],
        }
    }

    pub(super) fn network_menu(&self) -> GroupMenu<'_, Message> {
        let members = [self
            .widget_groups
            .net_interfaces_group
            .group_content(self.clock)
            .map(Message::NetInterfaces)]
        .into_iter()
        .chain(
            self.widget_groups
                .ethernet_ports_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(Message::EthernetPorts)),
        )
        .chain(
            self.widget_groups
                .bluetooth_adapters_group
                .group_contents(
                    &|hardware_address| {
                        self.widget_groups
                            .gamepad_group
                            .report_timing_for_address(hardware_address)
                    },
                    self.clock,
                )
                .into_iter()
                .map(|group| group.map(Message::BluetoothAdapters)),
        )
        .chain([
            self.widget_groups
                .reachability_group
                .group_content()
                .map(Message::Reachability),
            self.widget_groups
                .vpn_group
                .group_content()
                .map(Message::Vpn),
        ]);
        self.group_menu(
            GroupMenuKind::Network,
            "Network",
            Phosphor::Network(Weight::Regular),
            members,
        )
    }

    pub(super) fn hardware_menu(&self) -> GroupMenu<'_, Message> {
        let config = &self.config;
        let members = [
            self.widget_groups
                .power_group
                .group_content(config)
                .map(Message::Power),
            self.widget_groups
                .compute_group
                .group_content()
                .map(Message::Compute),
            self.widget_groups
                .memory_board_group
                .group_content()
                .map(Message::MemoryBoard),
        ]
        .into_iter()
        .chain(
            self.widget_groups
                .gpu_group
                .group_contents()
                .map(|group| group.map(Message::Gpu)),
        )
        .chain(
            self.widget_groups
                .storage_space_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(Message::Storage)),
        )
        .chain(
            self.widget_groups
                .physical_drive_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(Message::Drive)),
        );
        self.group_menu(
            GroupMenuKind::Hardware,
            "Hardware",
            Phosphor::Cpu(Weight::Regular),
            members,
        )
    }

    pub(super) fn agenda_menu(&self) -> GroupMenu<'_, Message> {
        let config = &self.config;
        self.group_menu(
            GroupMenuKind::Agenda,
            "Agenda",
            Phosphor::CalendarBlank(Weight::Regular),
            [
                self.widget_groups
                    .caldav_group
                    .group_content()
                    .map(Message::Caldav),
                self.widget_groups
                    .quiet_schedule_group
                    .group_content(config)
                    .map(Message::QuietSchedule),
                self.widget_groups
                    .session_focus_group
                    .group_content(config)
                    .map(Message::SessionFocus),
            ],
        )
    }

    pub(super) fn media_menu(&self) -> GroupMenu<'_, Message> {
        let config = &self.config;
        let media_groups = self
            .widget_groups
            .media_group
            .group_contents(config, self.clock)
            .map(|group| group.map(Message::Media));
        let gamepad_groups = self
            .widget_groups
            .gamepad_group
            .group_contents(&config.widget_groups.gamepad, focused_app_id(&self.niri))
            .into_iter()
            .map(|group| group.map(Message::Gamepad));
        let members = media_groups
            .chain([
                self.widget_groups
                    .audio_group
                    .group_content(config)
                    .map(Message::Audio),
                self.widget_groups
                    .voice_dictation_group
                    .group_content()
                    .map(Message::VoiceDictation),
            ])
            .chain(gamepad_groups);
        self.group_menu(
            GroupMenuKind::Media,
            "Media",
            Phosphor::MusicNotes(Weight::Regular),
            members,
        )
    }
}
