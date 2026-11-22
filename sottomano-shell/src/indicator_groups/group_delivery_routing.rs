use iced::Task;
use sottomano_toml_config::Config;
use sottomano_widgets::clock::Clock;

use crate::{indicator_groups::ShellIndicatorGroups, shell_message::Message};

impl ShellIndicatorGroups {
    pub fn route(&mut self, message: Message, config: &Config, clock: Clock) -> Task<Message> {
        match message {
            Message::Weather(delivery) => self
                .weather_group
                .update(delivery, clock)
                .map(Message::Weather),
            Message::Location(delivery) => self
                .location_group
                .update(delivery, clock)
                .map(Message::Location),
            Message::Wallpaper(delivery) => self
                .wallpaper_group
                .update(delivery, clock)
                .map(Message::Wallpaper),
            Message::Caldav(delivery) => self
                .caldav_group
                .update(delivery, clock)
                .map(Message::Caldav),
            Message::Power(delivery) => {
                self.power_group.update(delivery, clock).map(Message::Power)
            }
            Message::Compute(delivery) => self
                .compute_group
                .update(delivery, clock)
                .map(Message::Compute),
            Message::MemoryBoard(delivery) => self
                .memory_board_group
                .update(delivery, clock)
                .map(Message::MemoryBoard),
            Message::Gpu(delivery) => self.gpu_group.update(delivery, clock).map(Message::Gpu),
            Message::Storage(delivery) => self
                .storage_space_group
                .update(delivery, clock)
                .map(Message::Storage),
            Message::Drive(delivery) => self
                .physical_drive_group
                .update(delivery, clock)
                .map(Message::Drive),
            Message::SidebarSettings(delivery) => self
                .sidebar_settings_group
                .update(delivery)
                .map(Message::SidebarSettings),
            Message::CompositorStyling(delivery) => self
                .compositor_styling_group
                .update(delivery)
                .map(Message::CompositorStyling),
            Message::Clock(delivery) => {
                self.clock_group.update(delivery, clock).map(Message::Clock)
            }
            Message::Monitor(delivery) => self
                .monitor_group
                .update(delivery, clock)
                .map(Message::Monitor),
            Message::ColorTemperature(delivery) => self
                .color_temperature_group
                .update(delivery, clock)
                .map(Message::ColorTemperature),
            Message::DarkMode(delivery) => self
                .dark_mode_group
                .update(delivery, clock)
                .map(Message::DarkMode),
            Message::VoiceDictation(delivery) => self
                .voice_dictation_group
                .update(delivery, clock)
                .map(Message::VoiceDictation),
            Message::Audio(delivery) => {
                self.audio_group.update(delivery, clock).map(Message::Audio)
            }
            connectivity_delivery => self.route_connectivity(connectivity_delivery, config, clock),
        }
    }

    fn route_connectivity(
        &mut self,
        message: Message,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        match message {
            Message::WifiAdapters(delivery) => self
                .wifi_adapters_group
                .update(delivery, clock)
                .map(Message::WifiAdapters),
            Message::MobileModems(delivery) => self
                .mobile_modems_group
                .update(delivery, clock)
                .map(Message::MobileModems),
            Message::SteamClient(delivery) => self
                .steam_client_groups
                .update(delivery, clock)
                .map(Message::SteamClient),
            Message::EthernetPorts(delivery) => self
                .ethernet_ports_group
                .update(delivery, clock)
                .map(Message::EthernetPorts),
            Message::BluetoothAdapters(delivery) => self
                .bluetooth_adapters_group
                .update(delivery, clock)
                .map(Message::BluetoothAdapters),
            Message::Reachability(delivery) => self
                .reachability_group
                .update(delivery, &config.indicator_groups.reachability, clock)
                .map(Message::Reachability),
            Message::Tailnet(delivery) => self
                .tailnet_group
                .update(delivery, clock)
                .map(Message::Tailnet),
            Message::WireguardTunnels(delivery) => self
                .wireguard_tunnels_group
                .update(delivery, clock)
                .map(Message::WireguardTunnels),
            Message::NetworkManagerVpns(delivery) => self
                .network_manager_vpn_group
                .update(delivery, clock)
                .map(Message::NetworkManagerVpns),
            Message::SystemdManager(delivery) => self
                .systemd_manager_group
                .update(delivery, clock)
                .map(Message::SystemdManager),
            Message::JournalEvents(delivery) => self
                .journal_events_group
                .update(delivery, clock)
                .map(Message::JournalEvents),
            Message::PendingUpgrades(delivery) => self
                .pending_upgrades_group
                .update(delivery, clock)
                .map(Message::PendingUpgrades),
            Message::ClaudeUsage(delivery) => self
                .claude_usage_group
                .update(delivery, clock)
                .map(Message::ClaudeUsage),
            Message::CopilotUsage(delivery) => self
                .copilot_usage_group
                .update(delivery, clock)
                .map(Message::CopilotUsage),
            Message::OpenRouterCredit(delivery) => self
                .openrouter_credit_group
                .update(delivery, clock)
                .map(Message::OpenRouterCredit),
            Message::LlamaServer(delivery) => self
                .llama_server_group
                .update(delivery, config, clock)
                .map(Message::LlamaServer),
            Message::CiRuns(delivery) => self
                .ci_runs_group
                .update(delivery, clock)
                .map(Message::CiRuns),
            Message::SessionFocus(delivery) => self
                .session_focus_group
                .update(delivery, &config.indicator_groups.session_focus, clock)
                .map(Message::SessionFocus),
            Message::LoginSession(delivery) => self
                .login_session_group
                .update(delivery, clock)
                .map(Message::LoginSession),
            guest_delivery => self.route_guests(guest_delivery, clock),
        }
    }

    fn route_guests(&mut self, message: Message, clock: Clock) -> Task<Message> {
        match message {
            Message::PodmanEngines(delivery) => self
                .podman_engines_group
                .update(delivery, clock)
                .map(Message::PodmanEngines),
            Message::NspawnContainers(delivery) => self
                .nspawn_containers_group
                .update(delivery, clock)
                .map(Message::NspawnContainers),
            Message::CgroupMemoryLimits(delivery) => self
                .cgroup_memory_limits_group
                .update(delivery, clock)
                .map(Message::CgroupMemoryLimits),
            Message::DockerEngines(delivery) => self
                .docker_engines_group
                .update(delivery, clock)
                .map(Message::DockerEngines),
            Message::ApptainerContainers(delivery) => self
                .apptainer_containers_group
                .update(delivery, clock)
                .map(Message::ApptainerContainers),
            Message::VmwareMachines(delivery) => self
                .vmware_machines_group
                .update(delivery, clock)
                .map(Message::VmwareMachines),
            Message::VirtualBoxMachines(delivery) => self
                .virtualbox_machines_group
                .update(delivery, clock)
                .map(Message::VirtualBoxMachines),
            Message::WaydroidContainer(delivery) => self
                .waydroid_container_group
                .update(delivery, clock)
                .map(Message::WaydroidContainer),
            Message::FlatpakSandboxes(delivery) => self
                .flatpak_sandboxes_group
                .update(delivery, clock)
                .map(Message::FlatpakSandboxes),
            Message::LxcContainers(delivery) => self
                .lxc_containers_group
                .update(delivery, clock)
                .map(Message::LxcContainers),
            Message::IncusInstances(delivery) => self
                .incus_instances_group
                .update(delivery, clock)
                .map(Message::IncusInstances),
            Message::MicrovmGuests(delivery) => self
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
