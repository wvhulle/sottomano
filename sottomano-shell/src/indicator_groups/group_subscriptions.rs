use iced::Subscription;
use sottomano_desktop_sources::{
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;
use sottomano_widgets::{
    background_work::snapshot_subscription,
    group_menus::{group_menu_state::MenuTopicPresence, group_source_demand::GroupSourceDemand},
    indicator_groups::{
        audio_group::AudioGroup,
        bluetooth_adapters_group::BluetoothAdaptersGroup,
        compute_group::ComputeMessage,
        copilot_usage_group::CopilotUsageGroup,
        dark_mode_group::DarkModeGroup,
        idle_pause_groups::IdlePauseGroups,
        indicator_group_identity::IndicatorGroupFamily,
        journal_events_group::JournalEventsGroup,
        login_session_group::LoginSessionGroup,
        memory_board_group::{MemoryBoardGroup, MemoryBoardMessage},
        mobile_modems_group::MobileModemsGroup,
        monitor_group::MonitorGroup,
        pending_upgrades_group::PendingUpgradesGroup,
        power_group::PowerGroup,
        quiet_schedule_group::QuietScheduleGroup,
        session_focus_group::SessionFocusGroup,
        steam_client_groups::SteamClientGroups,
        systemd_manager_group::SystemdManagerGroup,
        tray_group::TrayGroup,
        virtual_machines_group::VirtualMachinesGroup,
        window_group::WindowGroup,
    },
};

use crate::{indicator_groups::ShellIndicatorGroups, shell_message::Message};

impl ShellIndicatorGroups {
    pub fn subscription(
        &self,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<Message> {
        let working_days = self.quiet_schedule_group.working_days;
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            self.weather_group
                .subscription(buses, source_demand(IndicatorGroupFamily::Weather))
                .map(Message::Weather),
            self.wallpaper_group
                .subscription(buses)
                .map(Message::Wallpaper),
            self.caldav_group
                .subscription(&config.indicator_groups.caldav, buses)
                .map(Message::Caldav),
            self.compute_group
                .subscription(
                    &config.indicator_groups.compute.temperature_sensors,
                    source_demand(IndicatorGroupFamily::Compute),
                )
                .map(Message::Compute),
            MemoryBoardGroup::subscription(&config.indicator_groups.memory_board.growth_trend)
                .map(Message::MemoryBoard),
            self.gpu_group.subscription().map(Message::Gpu),
            self.storage_space_group
                .subscription()
                .map(Message::Storage),
            self.physical_drive_group
                .subscription(
                    &config.indicator_groups.physical_drive.temperature_sensors,
                    source_demand(IndicatorGroupFamily::PhysicalDrive),
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
                .subscription(&config.indicator_groups.gamepad)
                .map(Message::Gamepad),
            self.claude_usage_group
                .subscription(config, working_days, buses)
                .map(Message::ClaudeUsage),
            CopilotUsageGroup::subscription(config, working_days).map(Message::CopilotUsage),
            self.openrouter_credit_group
                .subscription(config, buses)
                .map(Message::OpenRouterCredit),
            WindowGroup::subscription(&config.indicator_groups.window.agents)
                .map(Message::WindowGroup),
            SessionFocusGroup::subscription(&config.indicator_groups.session_focus)
                .map(Message::SessionFocus),
            self.incus_instances_group
                .subscription(source_demand(IndicatorGroupFamily::IncusInstances))
                .map(Message::IncusInstances),
            self.lxc_containers_group
                .subscription(source_demand(IndicatorGroupFamily::LxcContainers))
                .map(Message::LxcContainers),
            self.flatpak_sandboxes_group
                .subscription(source_demand(IndicatorGroupFamily::FlatpakSandboxes))
                .map(Message::FlatpakSandboxes),
            self.virtualbox_machines_group
                .subscription(source_demand(IndicatorGroupFamily::VirtualBoxMachines))
                .map(Message::VirtualBoxMachines),
            self.vmware_machines_group
                .subscription(source_demand(IndicatorGroupFamily::VmwareMachines))
                .map(Message::VmwareMachines),
            self.apptainer_containers_group
                .subscription(source_demand(IndicatorGroupFamily::ApptainerContainers))
                .map(Message::ApptainerContainers),
            self.docker_engines_group
                .subscription(source_demand(IndicatorGroupFamily::DockerEngines))
                .map(Message::DockerEngines),
            self.cgroup_memory_limits_group
                .subscription()
                .map(Message::CgroupMemoryLimits),
        ])
    }

    pub fn desktop_bus_subscription(
        &self,
        config: &Config,
        buses: &DesktopBuses,
        window_menu_bar_presence: MenuTopicPresence,
    ) -> Subscription<Message> {
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            PowerGroup::subscription(buses).map(Message::Power),
            self.window_menu_bar
                .subscription(buses, window_menu_bar_presence)
                .map(Message::WindowMenuBar),
            LoginSessionGroup::subscription(buses).map(Message::LoginSession),
            IdlePauseGroups::subscription(
                buses,
                config.indicator_groups.session_focus.idle_detection_delay,
            )
            .map(Message::IdleTimeouts),
            SystemdManagerGroup::subscription(&config.indicator_groups.systemd_manager, buses)
                .map(Message::SystemdManager),
            PendingUpgradesGroup::subscription(buses).map(Message::PendingUpgrades),
            self.virtualization_bus_subscriptions(buses),
            self.ci_runs_group.subscription(buses).map(Message::CiRuns),
            self.reachability_group
                .subscription(
                    &config.indicator_groups.reachability,
                    buses,
                    source_demand(IndicatorGroupFamily::Reachability),
                )
                .map(Message::Reachability),
            self.tailnet_group
                .subscription(&source_demand(IndicatorGroupFamily::Tailnet))
                .map(Message::Tailnet),
            self.wireguard_tunnels_group
                .subscription(
                    buses,
                    &source_demand(IndicatorGroupFamily::WireguardTunnels),
                )
                .map(Message::WireguardTunnels),
            self.network_manager_vpn_group
                .subscription(
                    buses,
                    &source_demand(IndicatorGroupFamily::NetworkManagerVpns),
                )
                .map(Message::NetworkManagerVpns),
            self.wifi_adapters_group
                .subscription(buses, source_demand(IndicatorGroupFamily::WifiAdapters))
                .map(Message::WifiAdapters),
            MobileModemsGroup::subscription(buses).map(Message::MobileModems),
            SteamClientGroups::subscription().map(Message::SteamClient),
            self.ethernet_ports_group
                .subscription(buses, source_demand(IndicatorGroupFamily::EthernetPorts))
                .map(Message::EthernetPorts),
            BluetoothAdaptersGroup::subscription(buses).map(Message::BluetoothAdapters),
            self.gamepad_group
                .network_subscription(buses, source_demand(IndicatorGroupFamily::Gamepad))
                .map(Message::Gamepad),
            MonitorGroup::subscription(buses).map(Message::Monitor),
            self.media_group
                .subscription(
                    buses,
                    &config.indicator_groups.media.lyrics,
                    &source_demand(IndicatorGroupFamily::Media),
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
                .map(|delivery| Message::Compute(ComputeMessage::HostHardware(Box::new(delivery)))),
            snapshot_subscription::<HostHardware>(buses.clone()).map(|delivery| {
                Message::MemoryBoard(MemoryBoardMessage::Hardware(Box::new(delivery)))
            }),
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

    fn virtualization_bus_subscriptions(&self, buses: &DesktopBuses) -> Subscription<Message> {
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            self.podman_engines_group
                .subscription(buses)
                .map(Message::PodmanEngines),
            VirtualMachinesGroup::subscription(buses).map(Message::VirtualMachines),
            self.waydroid_container_group
                .subscription(
                    buses,
                    source_demand(IndicatorGroupFamily::WaydroidContainer),
                )
                .map(Message::WaydroidContainer),
            self.nspawn_containers_group
                .subscription(buses)
                .map(Message::NspawnContainers),
            self.microvm_guests_group
                .subscription(source_demand(IndicatorGroupFamily::MicrovmGuests), buses)
                .map(Message::MicrovmGuests),
        ])
    }
}
