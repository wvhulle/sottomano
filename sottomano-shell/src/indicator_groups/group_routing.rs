use iced::Task;
use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock, group_indicators::indicator_group_interaction::IndicatorGroupInteraction,
};

use crate::{indicator_groups::ShellIndicatorGroups, shell_message::Message};

impl ShellIndicatorGroups {
    pub fn interact(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        let menus = &mut self.panel_menus;
        match group_interaction {
            IndicatorGroupInteraction::Weather(interaction) => menus
                .agenda
                .weather_group
                .interact(interaction)
                .map(Message::Weather),
            IndicatorGroupInteraction::Location(interaction) => menus
                .agenda
                .location_group
                .interact(interaction)
                .map(Message::Location),
            IndicatorGroupInteraction::Wallpaper(expansion) => menus
                .display
                .wallpaper_group
                .interact(expansion)
                .map(Message::Wallpaper),
            IndicatorGroupInteraction::Caldav(expansion) => menus
                .agenda
                .caldav_group
                .interact(expansion)
                .map(Message::Caldav),
            IndicatorGroupInteraction::SidebarSettings(message) => menus
                .appearance
                .sidebar_settings_group
                .update(message)
                .map(Message::SidebarSettings),
            IndicatorGroupInteraction::CompositorStyling(message) => menus
                .appearance
                .compositor_styling_group
                .update(message)
                .map(Message::CompositorStyling),
            IndicatorGroupInteraction::Clock(interaction) => menus
                .agenda
                .clock_group
                .interact(interaction)
                .map(Message::Clock),
            IndicatorGroupInteraction::Monitor(interaction) => menus
                .display
                .monitor_group
                .interact(interaction)
                .map(Message::Monitor),
            IndicatorGroupInteraction::ColorTemperature(interaction) => menus
                .display
                .color_temperature_group
                .interact(interaction)
                .map(Message::ColorTemperature),
            IndicatorGroupInteraction::DarkMode(interaction) => menus
                .display
                .dark_mode_group
                .interact(interaction, clock)
                .map(Message::DarkMode),
            IndicatorGroupInteraction::VoiceDictation(interaction) => menus
                .media
                .voice_dictation_group
                .interact(interaction, config, clock)
                .map(Message::VoiceDictation),
            IndicatorGroupInteraction::Audio(message) => menus
                .media
                .audio_group
                .update(message, clock)
                .map(Message::Audio),
            IndicatorGroupInteraction::IdleTimeouts(interaction) => menus
                .session
                .idle_pause_groups
                .interact(interaction)
                .map(Message::IdleTimeouts),
            hardware_interaction => self.interact_hardware(hardware_interaction, config, clock),
        }
    }
    fn interact_hardware(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        let menus = &mut self.panel_menus;
        match group_interaction {
            IndicatorGroupInteraction::Power(interaction) => menus
                .hardware
                .power_group
                .interact(interaction, config, clock)
                .map(Message::Power),
            IndicatorGroupInteraction::Compute(interaction) => menus
                .hardware
                .compute_group
                .interact(interaction, config, clock)
                .map(Message::Compute),
            IndicatorGroupInteraction::MemoryBoard(interaction) => menus
                .hardware
                .memory_board_group
                .interact(interaction, config, clock)
                .map(Message::MemoryBoard),
            IndicatorGroupInteraction::Gpu(interaction) => menus
                .hardware
                .gpu_group
                .interact(interaction, config, clock)
                .map(Message::Gpu),
            IndicatorGroupInteraction::Storage(interaction) => menus
                .hardware
                .storage_space_group
                .interact(interaction)
                .map(Message::Storage),
            IndicatorGroupInteraction::Drive(interaction) => menus
                .hardware
                .physical_drive_group
                .interact(interaction, config)
                .map(Message::Drive),
            connectivity_interaction => {
                self.interact_connectivity(connectivity_interaction, config, clock)
            }
        }
    }

    fn interact_connectivity(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        let menus = &mut self.panel_menus;
        match group_interaction {
            IndicatorGroupInteraction::WifiAdapters(interaction) => menus
                .network
                .wifi_adapters_group
                .interact(interaction)
                .map(Message::WifiAdapters),
            IndicatorGroupInteraction::MobileModems(interaction) => menus
                .network
                .mobile_modems_group
                .interact(interaction)
                .map(Message::MobileModems),
            IndicatorGroupInteraction::SteamClient(interaction) => menus
                .gaming
                .steam_client_groups
                .interact(interaction)
                .map(Message::SteamClient),
            IndicatorGroupInteraction::EthernetPorts(interaction) => menus
                .network
                .ethernet_ports_group
                .interact(interaction)
                .map(Message::EthernetPorts),
            IndicatorGroupInteraction::BluetoothAdapters(interaction) => menus
                .network
                .bluetooth_adapters_group
                .interact(interaction)
                .map(Message::BluetoothAdapters),
            IndicatorGroupInteraction::Reachability(interaction) => menus
                .network
                .reachability_group
                .interact(interaction)
                .map(Message::Reachability),
            IndicatorGroupInteraction::Tailnet(interaction) => menus
                .network
                .tailnet_group
                .interact(interaction, config, clock)
                .map(Message::Tailnet),
            IndicatorGroupInteraction::WireguardTunnels(interaction) => menus
                .network
                .wireguard_tunnels_group
                .interact(interaction)
                .map(Message::WireguardTunnels),
            IndicatorGroupInteraction::NetworkManagerVpns(interaction) => menus
                .network
                .network_manager_vpn_group
                .interact(interaction)
                .map(Message::NetworkManagerVpns),
            IndicatorGroupInteraction::SessionFocus(message) => menus
                .agenda
                .session_focus_group
                .update(message, &config.panel_menus.agenda.session_focus, clock)
                .map(Message::SessionFocus),
            IndicatorGroupInteraction::LoginSession(interaction) => menus
                .session
                .login_session_group
                .interact(interaction, config, clock)
                .map(Message::LoginSession),
            services_interaction => self.interact_services(services_interaction, config, clock),
        }
    }
    fn interact_services(
        &mut self,
        group_interaction: IndicatorGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        let menus = &mut self.panel_menus;
        match group_interaction {
            IndicatorGroupInteraction::SystemdManager(interaction) => menus
                .services
                .systemd_manager_group
                .interact(interaction)
                .map(Message::SystemdManager),
            IndicatorGroupInteraction::JournalEvents(interaction) => menus
                .services
                .journal_events_group
                .interact(interaction)
                .map(Message::JournalEvents),
            IndicatorGroupInteraction::PendingUpgrades(interaction) => menus
                .services
                .pending_upgrades_group
                .interact(interaction)
                .map(Message::PendingUpgrades),
            IndicatorGroupInteraction::ClaudeUsage(interaction) => menus
                .services
                .claude_usage_group
                .interact(interaction, config)
                .map(Message::ClaudeUsage),
            IndicatorGroupInteraction::CopilotUsage(interaction) => {
                let working_days = menus.agenda.quiet_schedule_group.working_days;
                menus
                    .services
                    .copilot_usage_group
                    .interact(interaction, config, working_days)
                    .map(Message::CopilotUsage)
            }
            IndicatorGroupInteraction::OpenRouterCredit(interaction) => menus
                .services
                .openrouter_credit_group
                .interact(interaction)
                .map(Message::OpenRouterCredit),
            IndicatorGroupInteraction::LlamaServer(interaction) => menus
                .services
                .llama_server_group
                .interact(interaction, config, clock)
                .map(Message::LlamaServer),
            IndicatorGroupInteraction::CiRuns(interaction) => menus
                .services
                .ci_runs_group
                .interact(interaction)
                .map(Message::CiRuns),
            guest_interaction => self.interact_guests(guest_interaction),
        }
    }

    fn interact_guests(&mut self, group_interaction: IndicatorGroupInteraction) -> Task<Message> {
        match group_interaction {
            IndicatorGroupInteraction::PodmanEngines(interaction) => self
                .panel_menus
                .virtualization
                .podman_engines_group
                .interact(interaction)
                .map(Message::PodmanEngines),
            IndicatorGroupInteraction::NspawnContainers(interaction) => self
                .panel_menus
                .virtualization
                .nspawn_containers_group
                .interact(interaction)
                .map(Message::NspawnContainers),
            IndicatorGroupInteraction::CgroupMemoryLimits(interaction) => self
                .panel_menus
                .virtualization
                .cgroup_memory_limits_group
                .interact(interaction)
                .map(Message::CgroupMemoryLimits),
            IndicatorGroupInteraction::DockerEngines(interaction) => self
                .panel_menus
                .virtualization
                .docker_engines_group
                .interact(interaction)
                .map(Message::DockerEngines),
            IndicatorGroupInteraction::ApptainerContainers(interaction) => self
                .panel_menus
                .virtualization
                .apptainer_containers_group
                .interact(interaction)
                .map(Message::ApptainerContainers),
            IndicatorGroupInteraction::VmwareMachines(interaction) => self
                .panel_menus
                .virtualization
                .vmware_machines_group
                .interact(interaction)
                .map(Message::VmwareMachines),
            IndicatorGroupInteraction::VirtualBoxMachines(interaction) => self
                .panel_menus
                .virtualization
                .virtualbox_machines_group
                .interact(interaction)
                .map(Message::VirtualBoxMachines),
            IndicatorGroupInteraction::WaydroidContainer(interaction) => self
                .panel_menus
                .virtualization
                .waydroid_container_group
                .interact(interaction)
                .map(Message::WaydroidContainer),
            IndicatorGroupInteraction::FlatpakSandboxes(interaction) => self
                .panel_menus
                .virtualization
                .flatpak_sandboxes_group
                .interact(interaction)
                .map(Message::FlatpakSandboxes),
            IndicatorGroupInteraction::LxcContainers(interaction) => self
                .panel_menus
                .virtualization
                .lxc_containers_group
                .interact(interaction)
                .map(Message::LxcContainers),
            IndicatorGroupInteraction::IncusInstances(interaction) => self
                .panel_menus
                .virtualization
                .incus_instances_group
                .interact(interaction)
                .map(Message::IncusInstances),
            IndicatorGroupInteraction::MicrovmGuests(interaction) => self
                .panel_menus
                .virtualization
                .microvm_guests_group
                .interact(interaction)
                .map(Message::MicrovmGuests),
            unrouted => {
                tracing::warn!(?unrouted, "not an indicator group interaction");
                Task::none()
            }
        }
    }
}
