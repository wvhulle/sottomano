use iced::Element;
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    widget_groups::{
        notification_group::NotificationStatus, widget_group_content::WidgetGroupContent,
    },
};

use crate::{
    desktop_shell::{DesktopShell, gamepad_window_claim},
    shell_message::Message,
};

pub(super) struct TopBarGroups<'a> {
    pub start_menus: [GroupMenuView<'a, Message>; 2],
    pub center: [WidgetGroupContent<'a, Message>; 1],
    pub agenda_group_menu: GroupMenuView<'a, Message>,
    pub notifications: WidgetGroupContent<'a, Message>,
    pub media_group_menu: GroupMenuView<'a, Message>,
    pub end: [WidgetGroupContent<'a, Message>; 1],
}

impl DesktopShell {
    pub(super) fn top_bar<'a>(&'a self) -> Element<'a, Message> {
        let config = &self.config;
        let TopBarGroups {
            start_menus,
            center,
            agenda_group_menu,
            notifications,
            media_group_menu,
            end,
        } = self.top_bar_groups();
        let view =
            |group: WidgetGroupContent<'a, Message>| group.view(config, Position::Top, self.clock);
        self.panel_bar(
            Position::Top,
            start_menus
                .into_iter()
                .flat_map(|group_menu| group_menu.panel_views(config, Position::Top, self.clock)),
            center
                .map(view)
                .into_iter()
                .chain(agenda_group_menu.panel_views(config, Position::Top, self.clock))
                .chain([view(notifications)]),
            media_group_menu
                .panel_views(config, Position::Top, self.clock)
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
        TopBarGroups {
            start_menus: [self.hardware_group_menu(), self.network_group_menu()],
            center: [self
                .widget_groups
                .clock_group
                .group_content()
                .map(Message::Clock)],
            agenda_group_menu: self.agenda_group_menu(),
            notifications: self
                .widget_groups
                .notification_group
                .group_content(self.notification_status())
                .map(Message::NotificationGroup),
            media_group_menu: self.media_group_menu(),
            end: [self
                .widget_groups
                .session_power_group
                .group_content()
                .map(Message::Session)],
        }
    }

    pub(super) fn network_group_menu(&self) -> GroupMenuView<'_, Message> {
        let groups = self
            .widget_groups
            .wifi_adapters_group
            .group_contents()
            .into_iter()
            .map(|group| group.map(Message::WifiAdapters))
            .chain(
                self.widget_groups
                    .mobile_modems_group
                    .group_contents()
                    .into_iter()
                    .map(|group| group.map(Message::MobileModems)),
            )
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
        self.group_menu(GroupMenuTopic::Network, groups)
    }

    pub(super) fn hardware_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        let groups = [
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
        self.group_menu(GroupMenuTopic::Hardware, groups)
    }

    pub(super) fn agenda_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Agenda,
            [
                self.widget_groups
                    .caldav_group
                    .group_content(&config.widget_groups.caldav)
                    .map(Message::Caldav),
                self.widget_groups
                    .weather_group
                    .group_content(config)
                    .map(Message::Weather),
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

    pub(super) fn media_group_menu(&self) -> GroupMenuView<'_, Message> {
        let config = &self.config;
        let media_groups = self
            .widget_groups
            .media_group
            .group_contents(config, self.clock)
            .map(|group| group.map(Message::Media));
        let gamepad_groups = self
            .widget_groups
            .gamepad_group
            .group_contents(
                &config.widget_groups.gamepad,
                gamepad_window_claim(&self.niri, &self.outputs),
            )
            .into_iter()
            .map(|group| group.map(Message::Gamepad));
        let groups = media_groups
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
        self.group_menu(GroupMenuTopic::Media, groups)
    }
}
