use iced::Subscription;
use sottomano_desktop_sources::{
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;
use sottomano_widgets::{
    background_work::snapshot_subscription,
    group_indicators::indicator_group_identity::IndicatorGroupFamily,
    group_menus::{group_menu_state::MenuTopicPresence, group_source_demand::GroupSourceDemand},
    output_windows::OutputWindows,
    panel_menus::{
        agenda_menu::{
            quiet_schedule_group::QuietScheduleGroup, session_focus_group::SessionFocusGroup,
        },
        display_menu::{dark_mode_group::DarkModeGroup, monitor_group::MonitorGroup},
        gaming_menu::steam_client_groups::SteamClientGroups,
        hardware_menu::{
            compute_group::ComputeMessage,
            memory_board_group::{MemoryBoardGroup, MemoryBoardMessage},
            power_group::PowerGroup,
        },
        media_menu::audio_group::AudioGroup,
        network_menu::{
            bluetooth_adapters_group::BluetoothAdaptersGroup,
            mobile_modems_group::MobileModemsGroup,
        },
        services_menu::{
            copilot_usage_group::CopilotUsageGroup, journal_events_group::JournalEventsGroup,
            pending_upgrades_group::PendingUpgradesGroup,
            systemd_manager_group::SystemdManagerGroup,
        },
        session_menu::{
            idle_pause_groups::IdlePauseGroups, login_session_group::LoginSessionGroup,
            tray_group::TrayGroup,
        },
        virtualization_menu::virtual_machines_group::VirtualMachinesGroup,
    },
};

use crate::{indicator_groups::ShellIndicatorGroups, shell_message::Message};

impl ShellIndicatorGroups {
    pub fn subscription(
        &self,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<Message> {
        let menus = &self.panel_menus;
        let working_days = menus.agenda.quiet_schedule_group.working_days;
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            menus
                .agenda
                .weather_group
                .subscription(buses, source_demand(IndicatorGroupFamily::Weather))
                .map(Message::Weather),
            menus
                .agenda
                .location_group
                .subscription()
                .map(Message::Location),
            menus
                .display
                .wallpaper_group
                .subscription(buses)
                .map(Message::Wallpaper),
            menus
                .agenda
                .caldav_group
                .subscription(&config.panel_menus.agenda.caldav, buses)
                .map(Message::Caldav),
            menus
                .hardware
                .compute_group
                .subscription(
                    &config.panel_menus.hardware.compute.temperature_sensors,
                    source_demand(IndicatorGroupFamily::Compute),
                )
                .map(Message::Compute),
            MemoryBoardGroup::subscription(&config.panel_menus.hardware.memory_board.growth_trend)
                .map(Message::MemoryBoard),
            menus.hardware.gpu_group.subscription().map(Message::Gpu),
            menus
                .hardware
                .storage_space_group
                .subscription()
                .map(Message::Storage),
            menus
                .hardware
                .physical_drive_group
                .subscription(
                    &config
                        .panel_menus
                        .hardware
                        .physical_drive
                        .temperature_sensors,
                    source_demand(IndicatorGroupFamily::PhysicalDrive),
                )
                .map(Message::Drive),
            menus
                .agenda
                .clock_group
                .subscription(config, GroupSourceDemand::on_panel())
                .map(Message::Clock),
            JournalEventsGroup::subscription().map(Message::JournalEvents),
            DarkModeGroup::subscription().map(Message::DarkMode),
            TrayGroup::subscription().map(Message::Tray),
            AudioGroup::subscription().map(Message::Audio),
            menus
                .gaming
                .gamepad_group
                .subscription(&config.panel_menus.gaming.gamepad)
                .map(Message::Gamepad),
            menus
                .services
                .claude_usage_group
                .subscription(config, working_days, buses)
                .map(Message::ClaudeUsage),
            CopilotUsageGroup::subscription(config, working_days).map(Message::CopilotUsage),
            menus
                .services
                .openrouter_credit_group
                .subscription(config, buses)
                .map(Message::OpenRouterCredit),
            OutputWindows::subscription(&config.output_windows.agents).map(Message::OutputWindows),
            SessionFocusGroup::subscription(&config.panel_menus.agenda.session_focus)
                .map(Message::SessionFocus),
            self.virtualization_subscriptions(),
        ])
    }
    fn virtualization_subscriptions(&self) -> Subscription<Message> {
        let menus = &self.panel_menus;
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            menus
                .virtualization
                .incus_instances_group
                .subscription(source_demand(IndicatorGroupFamily::IncusInstances))
                .map(Message::IncusInstances),
            menus
                .virtualization
                .lxc_containers_group
                .subscription(source_demand(IndicatorGroupFamily::LxcContainers))
                .map(Message::LxcContainers),
            menus
                .virtualization
                .flatpak_sandboxes_group
                .subscription(source_demand(IndicatorGroupFamily::FlatpakSandboxes))
                .map(Message::FlatpakSandboxes),
            menus
                .virtualization
                .virtualbox_machines_group
                .subscription(source_demand(IndicatorGroupFamily::VirtualBoxMachines))
                .map(Message::VirtualBoxMachines),
            menus
                .virtualization
                .vmware_machines_group
                .subscription(source_demand(IndicatorGroupFamily::VmwareMachines))
                .map(Message::VmwareMachines),
            menus
                .virtualization
                .apptainer_containers_group
                .subscription(source_demand(IndicatorGroupFamily::ApptainerContainers))
                .map(Message::ApptainerContainers),
            menus
                .virtualization
                .docker_engines_group
                .subscription(source_demand(IndicatorGroupFamily::DockerEngines))
                .map(Message::DockerEngines),
            menus
                .virtualization
                .cgroup_memory_limits_group
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
        let menus = &self.panel_menus;
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            PowerGroup::subscription(buses).map(Message::Power),
            self.window_menu_bar
                .subscription(buses, window_menu_bar_presence)
                .map(Message::WindowMenuBar),
            LoginSessionGroup::subscription(buses).map(Message::LoginSession),
            IdlePauseGroups::subscription(
                buses,
                config.panel_menus.agenda.session_focus.idle_detection_delay,
            )
            .map(Message::IdleTimeouts),
            SystemdManagerGroup::subscription(&config.panel_menus.services.systemd_manager, buses)
                .map(Message::SystemdManager),
            PendingUpgradesGroup::subscription(buses).map(Message::PendingUpgrades),
            self.virtualization_bus_subscriptions(buses),
            menus
                .services
                .ci_runs_group
                .subscription(buses)
                .map(Message::CiRuns),
            SteamClientGroups::subscription().map(Message::SteamClient),
            MonitorGroup::subscription(buses).map(Message::Monitor),
            menus
                .media
                .media_group
                .subscription(
                    buses,
                    &config.panel_menus.media.media.lyrics,
                    &source_demand(IndicatorGroupFamily::Media),
                )
                .map(Message::Media),
            DarkModeGroup::resume_subscription(buses).map(Message::DarkMode),
            menus
                .media
                .voice_dictation_group
                .subscription(buses)
                .map(Message::VoiceDictation),
            QuietScheduleGroup::resume_subscription(buses).map(Message::QuietSchedule),
            SessionFocusGroup::sleep_subscription(buses).map(Message::SessionFocus),
            OutputWindows::launcher_entry_subscription(buses).map(Message::OutputWindows),
            snapshot_subscription::<HostHardware>(buses.clone())
                .map(|delivery| Message::Compute(ComputeMessage::HostHardware(Box::new(delivery)))),
            snapshot_subscription::<HostHardware>(buses.clone()).map(|delivery| {
                Message::MemoryBoard(MemoryBoardMessage::Hardware(Box::new(delivery)))
            }),
            menus
                .services
                .copilot_usage_group
                .account_subscription(buses)
                .map(Message::CopilotUsage),
            menus
                .display
                .color_temperature_group
                .subscription(buses)
                .map(Message::ColorTemperature),
            menus
                .services
                .llama_server_group
                .subscription(buses)
                .map(Message::LlamaServer),
            self.network_bus_subscriptions(config, buses),
        ])
    }
    fn network_bus_subscriptions(
        &self,
        config: &Config,
        buses: &DesktopBuses,
    ) -> Subscription<Message> {
        let menus = &self.panel_menus;
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            menus
                .network
                .reachability_group
                .subscription(
                    &config.panel_menus.network.reachability,
                    buses,
                    source_demand(IndicatorGroupFamily::Reachability),
                )
                .map(Message::Reachability),
            menus
                .network
                .tailnet_group
                .subscription(&source_demand(IndicatorGroupFamily::Tailnet))
                .map(Message::Tailnet),
            menus
                .network
                .wireguard_tunnels_group
                .subscription(
                    buses,
                    &source_demand(IndicatorGroupFamily::WireguardTunnels),
                )
                .map(Message::WireguardTunnels),
            menus
                .network
                .network_manager_vpn_group
                .subscription(
                    buses,
                    &source_demand(IndicatorGroupFamily::NetworkManagerVpns),
                )
                .map(Message::NetworkManagerVpns),
            menus
                .network
                .wifi_adapters_group
                .subscription(buses, source_demand(IndicatorGroupFamily::WifiAdapters))
                .map(Message::WifiAdapters),
            MobileModemsGroup::subscription(buses).map(Message::MobileModems),
            menus
                .network
                .ethernet_ports_group
                .subscription(buses, source_demand(IndicatorGroupFamily::EthernetPorts))
                .map(Message::EthernetPorts),
            BluetoothAdaptersGroup::subscription(buses).map(Message::BluetoothAdapters),
            menus
                .gaming
                .gamepad_group
                .network_subscription(buses, source_demand(IndicatorGroupFamily::Gamepad))
                .map(Message::Gamepad),
        ])
    }

    fn virtualization_bus_subscriptions(&self, buses: &DesktopBuses) -> Subscription<Message> {
        let source_demand = |family| self.group_menus.source_demand(family);
        Subscription::batch([
            self.panel_menus
                .virtualization
                .podman_engines_group
                .subscription(buses)
                .map(Message::PodmanEngines),
            VirtualMachinesGroup::subscription(buses).map(Message::VirtualMachines),
            self.panel_menus
                .virtualization
                .waydroid_container_group
                .subscription(
                    buses,
                    source_demand(IndicatorGroupFamily::WaydroidContainer),
                )
                .map(Message::WaydroidContainer),
            self.panel_menus
                .virtualization
                .nspawn_containers_group
                .subscription(buses)
                .map(Message::NspawnContainers),
            self.panel_menus
                .virtualization
                .microvm_guests_group
                .subscription(source_demand(IndicatorGroupFamily::MicrovmGuests), buses)
                .map(Message::MicrovmGuests),
        ])
    }
}
