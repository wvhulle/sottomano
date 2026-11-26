use iced::Task;
use sottomano_toml_config::Config;
use sottomano_widgets::clock::Clock;

use crate::{indicator_groups::ShellIndicatorGroups, shell_message::Message};

impl ShellIndicatorGroups {
    pub fn route(&mut self, message: Message, config: &Config, clock: Clock) -> Task<Message> {
        let menus = &mut self.panel_menus;
        match message {
            Message::Weather(delivery) => menus
                .agenda
                .weather_group
                .update(delivery, clock)
                .map(Message::Weather),
            Message::Location(delivery) => menus
                .agenda
                .location_group
                .update(delivery, clock)
                .map(Message::Location),
            Message::Wallpaper(delivery) => menus
                .display
                .wallpaper_group
                .update(delivery, clock)
                .map(Message::Wallpaper),
            Message::Caldav(delivery) => menus
                .agenda
                .caldav_group
                .update(delivery, clock)
                .map(Message::Caldav),
            Message::Power(delivery) => menus
                .hardware
                .power_group
                .update(delivery, clock)
                .map(Message::Power),
            Message::Compute(delivery) => menus
                .hardware
                .compute_group
                .update(delivery, clock)
                .map(Message::Compute),
            Message::MemoryBoard(delivery) => menus
                .hardware
                .memory_board_group
                .update(delivery, clock)
                .map(Message::MemoryBoard),
            Message::Gpu(delivery) => menus
                .hardware
                .gpu_group
                .update(delivery, clock)
                .map(Message::Gpu),
            Message::Storage(delivery) => menus
                .hardware
                .storage_space_group
                .update(delivery, clock)
                .map(Message::Storage),
            Message::Drive(delivery) => menus
                .hardware
                .physical_drive_group
                .update(delivery, clock)
                .map(Message::Drive),
            Message::SidebarSettings(delivery) => menus
                .appearance
                .sidebar_settings_group
                .update(delivery)
                .map(Message::SidebarSettings),
            Message::CompositorStyling(delivery) => menus
                .appearance
                .compositor_styling_group
                .update(delivery)
                .map(Message::CompositorStyling),
            Message::Clock(delivery) => menus
                .agenda
                .clock_group
                .update(delivery, clock)
                .map(Message::Clock),
            Message::Monitor(delivery) => menus
                .display
                .monitor_group
                .update(delivery, clock)
                .map(Message::Monitor),
            Message::ColorTemperature(delivery) => menus
                .display
                .color_temperature_group
                .update(delivery, clock)
                .map(Message::ColorTemperature),
            Message::DarkMode(delivery) => menus
                .display
                .dark_mode_group
                .update(delivery, clock)
                .map(Message::DarkMode),
            Message::VoiceDictation(delivery) => menus
                .media
                .voice_dictation_group
                .update(delivery, clock)
                .map(Message::VoiceDictation),
            Message::Audio(delivery) => menus
                .media
                .audio_group
                .update(delivery, clock)
                .map(Message::Audio),
            connectivity_delivery => self.route_connectivity(connectivity_delivery, config, clock),
        }
    }

    fn route_connectivity(
        &mut self,
        message: Message,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        let menus = &mut self.panel_menus;
        match message {
            Message::WifiAdapters(delivery) => menus
                .network
                .wifi_adapters_group
                .update(delivery, clock)
                .map(Message::WifiAdapters),
            Message::MobileModems(delivery) => menus
                .network
                .mobile_modems_group
                .update(delivery, clock)
                .map(Message::MobileModems),
            Message::SteamDownloads(delivery) => menus
                .gaming
                .steam_downloads_group
                .update(delivery, clock)
                .map(Message::SteamDownloads),
            Message::SteamGames(delivery) => menus
                .gaming
                .steam_games_group
                .update(delivery, clock)
                .map(Message::SteamGames),
            Message::EthernetPorts(delivery) => menus
                .network
                .ethernet_ports_group
                .update(delivery, clock)
                .map(Message::EthernetPorts),
            Message::BluetoothAdapters(delivery) => menus
                .network
                .bluetooth_adapters_group
                .update(delivery, clock)
                .map(Message::BluetoothAdapters),
            Message::Reachability(delivery) => menus
                .network
                .reachability_group
                .update(delivery, &config.panel_menus.network.reachability, clock)
                .map(Message::Reachability),
            Message::Tailnet(delivery) => menus
                .network
                .tailnet_group
                .update(delivery, clock)
                .map(Message::Tailnet),
            Message::WireguardTunnels(delivery) => menus
                .network
                .wireguard_tunnels_group
                .update(delivery, clock)
                .map(Message::WireguardTunnels),
            Message::NetworkManagerVpns(delivery) => menus
                .network
                .network_manager_vpn_group
                .update(delivery, clock)
                .map(Message::NetworkManagerVpns),
            Message::SystemdManager(delivery) => menus
                .services
                .systemd_manager_group
                .update(delivery, clock)
                .map(Message::SystemdManager),
            Message::JournalEvents(delivery) => menus
                .services
                .journal_events_group
                .update(delivery, clock)
                .map(Message::JournalEvents),
            Message::PendingUpgrades(delivery) => menus
                .services
                .pending_upgrades_group
                .update(delivery, clock)
                .map(Message::PendingUpgrades),
            Message::ClaudeUsage(delivery) => menus
                .services
                .claude_usage_group
                .update(delivery, clock)
                .map(Message::ClaudeUsage),
            Message::CopilotUsage(delivery) => menus
                .services
                .copilot_usage_group
                .update(delivery, clock)
                .map(Message::CopilotUsage),
            Message::OpenRouterCredit(delivery) => menus
                .services
                .openrouter_credit_group
                .update(delivery, clock)
                .map(Message::OpenRouterCredit),
            Message::LlamaServer(delivery) => menus
                .services
                .llama_server_group
                .update(delivery, config, clock)
                .map(Message::LlamaServer),
            Message::CiRuns(delivery) => menus
                .services
                .ci_runs_group
                .update(delivery, clock)
                .map(Message::CiRuns),
            Message::SessionFocus(delivery) => menus
                .agenda
                .session_focus_group
                .update(delivery, &config.panel_menus.agenda.session_focus, clock)
                .map(Message::SessionFocus),
            Message::LoginSession(delivery) => menus
                .session
                .login_session_group
                .update(delivery, clock)
                .map(Message::LoginSession),
            guest_delivery => self.route_guests(guest_delivery, clock),
        }
    }

    fn route_guests(&mut self, message: Message, clock: Clock) -> Task<Message> {
        match message {
            Message::PodmanEngines(delivery) => self
                .panel_menus
                .virtualization
                .podman_engines_group
                .update(delivery, clock)
                .map(Message::PodmanEngines),
            Message::NspawnContainers(delivery) => self
                .panel_menus
                .virtualization
                .nspawn_containers_group
                .update(delivery, clock)
                .map(Message::NspawnContainers),
            Message::CgroupMemoryLimits(delivery) => self
                .panel_menus
                .virtualization
                .cgroup_memory_limits_group
                .update(delivery, clock)
                .map(Message::CgroupMemoryLimits),
            Message::DockerEngines(delivery) => self
                .panel_menus
                .virtualization
                .docker_engines_group
                .update(delivery, clock)
                .map(Message::DockerEngines),
            Message::ApptainerContainers(delivery) => self
                .panel_menus
                .virtualization
                .apptainer_containers_group
                .update(delivery, clock)
                .map(Message::ApptainerContainers),
            Message::VmwareMachines(delivery) => self
                .panel_menus
                .virtualization
                .vmware_machines_group
                .update(delivery, clock)
                .map(Message::VmwareMachines),
            Message::VirtualBoxMachines(delivery) => self
                .panel_menus
                .virtualization
                .virtualbox_machines_group
                .update(delivery, clock)
                .map(Message::VirtualBoxMachines),
            Message::WaydroidContainer(delivery) => self
                .panel_menus
                .virtualization
                .waydroid_container_group
                .update(delivery, clock)
                .map(Message::WaydroidContainer),
            Message::FlatpakSandboxes(delivery) => self
                .panel_menus
                .virtualization
                .flatpak_sandboxes_group
                .update(delivery, clock)
                .map(Message::FlatpakSandboxes),
            Message::LxcContainers(delivery) => self
                .panel_menus
                .virtualization
                .lxc_containers_group
                .update(delivery, clock)
                .map(Message::LxcContainers),
            Message::IncusInstances(delivery) => self
                .panel_menus
                .virtualization
                .incus_instances_group
                .update(delivery, clock)
                .map(Message::IncusInstances),
            Message::MicrovmGuests(delivery) => self
                .panel_menus
                .virtualization
                .microvm_guests_group
                .update(delivery, clock)
                .map(Message::MicrovmGuests),
            unrouted => {
                tracing::warn!(?unrouted, "not an indicator group message");
                Task::none()
            }
        }
    }
}
