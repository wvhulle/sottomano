use iced::Task;
use sottomano_desktop_sources::system_protocol_clients::dbus_desktop_buses::DesktopBuses;
use sottomano_toml_config::Config;
use sottomano_widgets::{
    clock::Clock,
    widget_groups::{
        color_temperature_group::ColorTemperatureGroup,
        monitor_group::MonitorGroup,
        network_manager_vpn_group::NetworkManagerVpnGroup,
        reachability_group::ReachabilityGroup,
        voice_dictation_group::VoiceDictationGroup,
        widget_group_interaction::{WidgetGroupBusRequest, WidgetGroupInteraction},
    },
};

use crate::{shell_message::Message, widget_groups::ShellWidgetGroups};

impl ShellWidgetGroups {
    pub fn request_on_buses(
        &mut self,
        bus_request: WidgetGroupBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<Message> {
        match bus_request {
            WidgetGroupBusRequest::Weather(request) => self
                .weather_group
                .on_buses(request, buses)
                .map(Message::Weather),
            WidgetGroupBusRequest::Power(request) => self
                .power_group
                .on_buses(request, config, buses, clock)
                .map(Message::Power),
            WidgetGroupBusRequest::ClaudeUsage(request) => {
                let working_days = self.quiet_schedule_group.working_days();
                self.claude_usage_group
                    .on_buses(request, config, working_days, buses, clock)
                    .map(Message::ClaudeUsage)
            }
            WidgetGroupBusRequest::CopilotUsage(request) => self
                .copilot_usage_group
                .on_buses(request, buses)
                .map(Message::CopilotUsage),
            WidgetGroupBusRequest::OpenRouterCredit(request) => self
                .openrouter_credit_group
                .on_buses(request, config, buses)
                .map(Message::OpenRouterCredit),
            WidgetGroupBusRequest::LoginSession(request) => self
                .login_session_group
                .on_buses(request, config, buses, clock)
                .map(Message::LoginSession),
            WidgetGroupBusRequest::VoiceDictation(request) => {
                VoiceDictationGroup::on_buses(request, buses).map(Message::VoiceDictation)
            }
            WidgetGroupBusRequest::Reachability(request) => {
                ReachabilityGroup::on_buses(request, buses).map(Message::Reachability)
            }
            WidgetGroupBusRequest::Tailnet(request) => self
                .tailnet_group
                .on_buses(request, config, buses, clock)
                .map(Message::Tailnet),
            WidgetGroupBusRequest::WireguardTunnels(request) => self
                .wireguard_tunnels_group
                .on_buses(request, config, buses, clock)
                .map(Message::WireguardTunnels),
            WidgetGroupBusRequest::NetworkManagerVpns(request) => {
                NetworkManagerVpnGroup::on_buses(request, buses).map(Message::NetworkManagerVpns)
            }
            WidgetGroupBusRequest::WifiAdapters(request) => self
                .wifi_adapters_group
                .on_buses(request, config, buses, clock)
                .map(Message::WifiAdapters),
            WidgetGroupBusRequest::EthernetPorts(request) => self
                .ethernet_ports_group
                .on_buses(request, config, buses, clock)
                .map(Message::EthernetPorts),
            WidgetGroupBusRequest::Monitor(request) => {
                MonitorGroup::on_buses(request, buses).map(Message::Monitor)
            }
            WidgetGroupBusRequest::ColorTemperature(request) => {
                ColorTemperatureGroup::on_buses(request, buses).map(Message::ColorTemperature)
            }
        }
    }

    pub fn interact(
        &mut self,
        group_interaction: WidgetGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        match group_interaction {
            WidgetGroupInteraction::Weather(interaction) => self
                .weather_group
                .interact(interaction)
                .map(Message::Weather),
            WidgetGroupInteraction::Wallpaper(expansion) => self
                .wallpaper_group
                .interact(expansion)
                .map(Message::Wallpaper),
            WidgetGroupInteraction::Caldav(expansion) => {
                self.caldav_group.interact(expansion).map(Message::Caldav)
            }
            WidgetGroupInteraction::Power(interaction) => self
                .power_group
                .interact(interaction, config, clock)
                .map(Message::Power),
            WidgetGroupInteraction::Compute(interaction) => self
                .compute_group
                .interact(interaction, config, clock)
                .map(Message::Compute),
            WidgetGroupInteraction::MemoryBoard(interaction) => self
                .memory_board_group
                .interact(interaction, config, clock)
                .map(Message::MemoryBoard),
            WidgetGroupInteraction::Gpu(interaction) => self
                .gpu_group
                .interact(interaction, config, clock)
                .map(Message::Gpu),
            WidgetGroupInteraction::Storage(interaction) => self
                .storage_space_group
                .interact(interaction)
                .map(Message::Storage),
            WidgetGroupInteraction::Drive(interaction) => self
                .physical_drive_group
                .interact(interaction, config)
                .map(Message::Drive),
            WidgetGroupInteraction::SidebarSettings(message) => self
                .sidebar_settings_group
                .update(message)
                .map(Message::SidebarSettings),
            WidgetGroupInteraction::CompositorStyling(message) => self
                .compositor_styling_group
                .update(message)
                .map(Message::CompositorStyling),
            WidgetGroupInteraction::Clock(interaction) => {
                self.clock_group.interact(interaction).map(Message::Clock)
            }
            WidgetGroupInteraction::Monitor(interaction) => self
                .monitor_group
                .interact(interaction)
                .map(Message::Monitor),
            WidgetGroupInteraction::ColorTemperature(interaction) => self
                .color_temperature_group
                .interact(interaction)
                .map(Message::ColorTemperature),
            WidgetGroupInteraction::DarkMode(interaction) => self
                .dark_mode_group
                .interact(interaction, clock)
                .map(Message::DarkMode),
            WidgetGroupInteraction::VoiceDictation(interaction) => self
                .voice_dictation_group
                .interact(interaction, config, clock)
                .map(Message::VoiceDictation),
            WidgetGroupInteraction::Audio(message) => {
                self.audio_group.update(message, clock).map(Message::Audio)
            }
            WidgetGroupInteraction::IdleTimeouts(interaction) => self
                .idle_pause_groups
                .interact(interaction)
                .map(Message::IdleTimeouts),
            connectivity_interaction => {
                self.interact_connectivity(connectivity_interaction, config, clock)
            }
        }
    }

    fn interact_connectivity(
        &mut self,
        group_interaction: WidgetGroupInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        match group_interaction {
            WidgetGroupInteraction::WifiAdapters(interaction) => self
                .wifi_adapters_group
                .interact(interaction)
                .map(Message::WifiAdapters),
            WidgetGroupInteraction::MobileModems(interaction) => self
                .mobile_modems_group
                .interact(interaction)
                .map(Message::MobileModems),
            WidgetGroupInteraction::EthernetPorts(interaction) => self
                .ethernet_ports_group
                .interact(interaction)
                .map(Message::EthernetPorts),
            WidgetGroupInteraction::BluetoothAdapters(interaction) => self
                .bluetooth_adapters_group
                .interact(interaction)
                .map(Message::BluetoothAdapters),
            WidgetGroupInteraction::Reachability(interaction) => self
                .reachability_group
                .interact(interaction)
                .map(Message::Reachability),
            WidgetGroupInteraction::Tailnet(interaction) => self
                .tailnet_group
                .interact(interaction, config, clock)
                .map(Message::Tailnet),
            WidgetGroupInteraction::WireguardTunnels(interaction) => self
                .wireguard_tunnels_group
                .interact(interaction)
                .map(Message::WireguardTunnels),
            WidgetGroupInteraction::NetworkManagerVpns(interaction) => self
                .network_manager_vpn_group
                .interact(interaction)
                .map(Message::NetworkManagerVpns),
            WidgetGroupInteraction::SystemdManager(interaction) => self
                .systemd_manager_group
                .interact(interaction)
                .map(Message::SystemdManager),
            WidgetGroupInteraction::JournalEvents(interaction) => self
                .journal_events_group
                .interact(interaction)
                .map(Message::JournalEvents),
            WidgetGroupInteraction::PendingUpgrades(interaction) => self
                .pending_upgrades_group
                .interact(interaction)
                .map(Message::PendingUpgrades),
            WidgetGroupInteraction::ClaudeUsage(interaction) => self
                .claude_usage_group
                .interact(interaction, config)
                .map(Message::ClaudeUsage),
            WidgetGroupInteraction::CopilotUsage(interaction) => {
                let working_days = self.quiet_schedule_group.working_days();
                self.copilot_usage_group
                    .interact(interaction, config, working_days)
                    .map(Message::CopilotUsage)
            }
            WidgetGroupInteraction::OpenRouterCredit(interaction) => self
                .openrouter_credit_group
                .interact(interaction)
                .map(Message::OpenRouterCredit),
            WidgetGroupInteraction::LlamaServer(interaction) => self
                .llama_server_group
                .interact(interaction, config, clock)
                .map(Message::LlamaServer),
            WidgetGroupInteraction::CiRuns(interaction) => self
                .ci_runs_group
                .interact(interaction)
                .map(Message::CiRuns),
            WidgetGroupInteraction::SessionFocus(message) => self
                .session_focus_group
                .update(message, &config.widget_groups.session_focus, clock)
                .map(Message::SessionFocus),
            WidgetGroupInteraction::LoginSession(interaction) => self
                .login_session_group
                .interact(interaction, config, clock)
                .map(Message::LoginSession),
            guest_interaction => self.interact_guests(guest_interaction),
        }
    }

    fn interact_guests(&mut self, group_interaction: WidgetGroupInteraction) -> Task<Message> {
        match group_interaction {
            WidgetGroupInteraction::PodmanEngines(interaction) => self
                .podman_engines_group
                .interact(interaction)
                .map(Message::PodmanEngines),
            WidgetGroupInteraction::NspawnContainers(interaction) => self
                .nspawn_containers_group
                .interact(interaction)
                .map(Message::NspawnContainers),
            WidgetGroupInteraction::CgroupMemoryLimits(interaction) => self
                .cgroup_memory_limits_group
                .interact(interaction)
                .map(Message::CgroupMemoryLimits),
            WidgetGroupInteraction::DockerEngines(interaction) => self
                .docker_engines_group
                .interact(interaction)
                .map(Message::DockerEngines),
            WidgetGroupInteraction::ApptainerContainers(interaction) => self
                .apptainer_containers_group
                .interact(interaction)
                .map(Message::ApptainerContainers),
            WidgetGroupInteraction::VmwareMachines(interaction) => self
                .vmware_machines_group
                .interact(interaction)
                .map(Message::VmwareMachines),
            WidgetGroupInteraction::VirtualBoxMachines(interaction) => self
                .virtualbox_machines_group
                .interact(interaction)
                .map(Message::VirtualBoxMachines),
            WidgetGroupInteraction::WaydroidContainer(interaction) => self
                .waydroid_container_group
                .interact(interaction)
                .map(Message::WaydroidContainer),
            WidgetGroupInteraction::FlatpakSandboxes(interaction) => self
                .flatpak_sandboxes_group
                .interact(interaction)
                .map(Message::FlatpakSandboxes),
            WidgetGroupInteraction::LxcContainers(interaction) => self
                .lxc_containers_group
                .interact(interaction)
                .map(Message::LxcContainers),
            WidgetGroupInteraction::IncusInstances(interaction) => self
                .incus_instances_group
                .interact(interaction)
                .map(Message::IncusInstances),
            WidgetGroupInteraction::MicrovmGuests(interaction) => self
                .microvm_guests_group
                .interact(interaction)
                .map(Message::MicrovmGuests),
            unrouted => {
                tracing::warn!(?unrouted, "not a widget group interaction");
                Task::none()
            }
        }
    }

    pub fn route(&mut self, message: Message, config: &Config, clock: Clock) -> Task<Message> {
        match message {
            Message::Weather(delivery) => self
                .weather_group
                .update(delivery, clock)
                .map(Message::Weather),
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
                .update(delivery, &config.widget_groups.reachability, clock)
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
                .update(delivery, &config.widget_groups.session_focus, clock)
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
                tracing::warn!(?unrouted, "not a widget group message");
                Task::none()
            }
        }
    }
}
