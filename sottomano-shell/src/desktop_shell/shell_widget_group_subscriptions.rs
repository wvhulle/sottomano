use iced::Subscription;
use sottomano_desktop_sources::{
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;
use sottomano_widgets::{
    background_work::snapshot_subscription,
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        audio_group::AudioGroup, bluetooth_adapters_group::BluetoothAdaptersGroup,
        copilot_usage_group::CopilotUsageGroup, dark_mode_group::DarkModeGroup,
        idle_pause_groups::IdlePauseGroups, journal_events_group::JournalEventsGroup,
        login_session_group::LoginSessionGroup, memory_board_group::MemoryBoardGroup,
        mobile_modems_group::MobileModemsGroup, monitor_group::MonitorGroup,
        pending_upgrades_group::PendingUpgradesGroup, power_group::PowerGroup,
        quiet_schedule_group::QuietScheduleGroup, session_focus_group::SessionFocusGroup,
        systemd_manager_group::SystemdManagerGroup, tray_group::TrayGroup,
        virtual_machines_group::VirtualMachinesGroup, widget_group_identity::WidgetGroupFamily,
        window_group::WindowGroup,
    },
};

use crate::{desktop_shell::shell_widget_groups::ShellWidgetGroups, shell_message::Message};

impl ShellWidgetGroups {
    pub fn subscription(
        &self,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<Message> {
        let working_days = self.quiet_schedule_group.working_days();
        Subscription::batch([
            self.weather_group
                .subscription(
                    buses,
                    self.group_menus.source_demand(WidgetGroupFamily::Weather),
                )
                .map(Message::Weather),
            self.wallpaper_group
                .subscription(buses)
                .map(Message::Wallpaper),
            self.caldav_group
                .subscription(&config.widget_groups.caldav, buses)
                .map(Message::Caldav),
            self.compute_group
                .subscription(
                    &config.widget_groups.compute.temperature_sensors,
                    self.group_menus.source_demand(WidgetGroupFamily::Compute),
                )
                .map(Message::Compute),
            MemoryBoardGroup::subscription(&config.widget_groups.memory_board.growth_trend)
                .map(Message::MemoryBoard),
            self.gpu_group.subscription().map(Message::Gpu),
            self.storage_space_group
                .subscription()
                .map(Message::Storage),
            self.physical_drive_group
                .subscription(
                    &config.widget_groups.physical_drive.temperature_sensors,
                    self.group_menus
                        .source_demand(WidgetGroupFamily::PhysicalDrive),
                )
                .map(Message::Drive),
            self.clock_group
                .subscription(config, GroupSourceDemand::on_panel())
                .map(Message::Clock),
            JournalEventsGroup::subscription().map(Message::JournalEvents),
            DarkModeGroup::subscription().map(Message::DarkMode),
            TrayGroup::subscription().map(Message::Tray),
            AudioGroup::subscription().map(Message::Audio),
            self.gamepad_group
                .subscription(&config.widget_groups.gamepad)
                .map(Message::Gamepad),
            self.claude_usage_group
                .subscription(config, working_days, buses)
                .map(Message::ClaudeUsage),
            CopilotUsageGroup::subscription(config, working_days).map(Message::CopilotUsage),
            self.openrouter_credit_group
                .subscription(config, buses)
                .map(Message::OpenRouterCredit),
            WindowGroup::subscription(&config.widget_groups.window.agents)
                .map(Message::WindowGroup),
            SessionFocusGroup::subscription(&config.widget_groups.session_focus)
                .map(Message::SessionFocus),
        ])
    }

    pub fn desktop_bus_subscription(
        &self,
        config: &Config,
        buses: &DesktopBuses,
    ) -> Subscription<Message> {
        Subscription::batch([
            PowerGroup::subscription(buses).map(Message::Power),
            self.global_menu_group
                .subscription(
                    buses,
                    self.group_menus
                        .source_demand(WidgetGroupFamily::GlobalMenu),
                )
                .map(Message::GlobalMenu),
            LoginSessionGroup::subscription(buses).map(Message::LoginSession),
            IdlePauseGroups::subscription(buses).map(Message::IdleTimeouts),
            SystemdManagerGroup::subscription(&config.widget_groups.systemd_manager, buses)
                .map(Message::SystemdManager),
            PendingUpgradesGroup::subscription(buses).map(Message::PendingUpgrades),
            self.podman_engines_group
                .subscription(buses)
                .map(Message::PodmanEngines),
            VirtualMachinesGroup::subscription(buses).map(Message::VirtualMachines),
            self.nspawn_containers_group
                .subscription(buses)
                .map(Message::NspawnContainers),
            self.ci_runs_group.subscription(buses).map(Message::CiRuns),
            self.reachability_group
                .subscription(
                    &config.widget_groups.reachability,
                    buses,
                    self.group_menus
                        .source_demand(WidgetGroupFamily::Reachability),
                )
                .map(Message::Reachability),
            self.tailnet_group
                .subscription(&self.group_menus.source_demand(WidgetGroupFamily::Tailnet))
                .map(Message::Tailnet),
            self.wireguard_tunnels_group
                .subscription(
                    buses,
                    &self
                        .group_menus
                        .source_demand(WidgetGroupFamily::WireguardTunnels),
                )
                .map(Message::WireguardTunnels),
            self.network_manager_vpn_group
                .subscription(
                    buses,
                    &self
                        .group_menus
                        .source_demand(WidgetGroupFamily::NetworkManagerVpns),
                )
                .map(Message::NetworkManagerVpns),
            self.wifi_adapters_group
                .subscription(
                    buses,
                    self.group_menus
                        .source_demand(WidgetGroupFamily::WifiAdapters),
                )
                .map(Message::WifiAdapters),
            MobileModemsGroup::subscription(buses).map(Message::MobileModems),
            self.ethernet_ports_group
                .subscription(
                    buses,
                    self.group_menus
                        .source_demand(WidgetGroupFamily::EthernetPorts),
                )
                .map(Message::EthernetPorts),
            BluetoothAdaptersGroup::subscription(buses).map(Message::BluetoothAdapters),
            self.gamepad_group
                .network_subscription(
                    buses,
                    self.group_menus.source_demand(WidgetGroupFamily::Gamepad),
                )
                .map(Message::Gamepad),
            MonitorGroup::subscription(buses).map(Message::Monitor),
            self.media_group
                .subscription(
                    buses,
                    &config.widget_groups.media.lyrics,
                    &self.group_menus.source_demand(WidgetGroupFamily::Media),
                )
                .map(Message::Media),
            DarkModeGroup::resume_subscription(buses).map(Message::DarkMode),
            self.voice_dictation_group
                .subscription(buses)
                .map(Message::VoiceDictation),
            QuietScheduleGroup::resume_subscription(buses).map(Message::QuietSchedule),
            SessionFocusGroup::sleep_subscription(buses).map(Message::SessionFocus),
            WindowGroup::launcher_entry_subscription(buses).map(Message::WindowGroup),
            snapshot_subscription::<HostHardware>(buses.clone())
                .map(|delivery| Message::HostHardware(Box::new(delivery))),
            self.copilot_usage_group
                .account_subscription(buses)
                .map(Message::CopilotUsage),
            self.color_temperature_group
                .subscription(buses)
                .map(Message::ColorTemperature),
            self.llama_server_group
                .subscription(buses)
                .map(Message::LlamaServer),
        ])
    }
}
