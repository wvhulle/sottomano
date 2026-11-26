mod agenda_interaction_conversions;
mod guest_interaction_conversions;
mod hardware_interaction_conversions;
mod network_interaction_conversions;
mod settings_interaction_conversions;

use strum::IntoStaticStr;

use crate::{
    cascading_menus::{
        app_launcher_menu::AppLauncherMessage, window_menu_bar::WindowMenuBarInteraction,
    },
    group_indicators::indicator_group_content::IndicatorGroupExpansionMessage,
    group_menus::group_menu_state::GroupMenuMessage,
    output_windows::OutputWindowsInteraction,
    panel_menus::{
        agenda_menu::{
            clock_group::ClockInteraction,
            location_group::LocationInteraction,
            notification_group::NotificationGroupMessage,
            quiet_schedule_group::QuietScheduleMessage,
            session_focus_group::SessionFocusMessage,
            weather_group::{WeatherBusRequest, WeatherInteraction},
        },
        appearance_menu::{
            compositor_styling_group::CompositorStylingMessage,
            overlay_settings_group::OverlaySettingsMessage,
            panel_settings_group::PanelSettingsMessage,
            sidebar_settings_group::SidebarSettingsMessage,
        },
        behaviour_menu::appearance_presets_group::AppearancePresetsMessage,
        display_menu::{
            color_temperature_group::{ColorTemperatureBusRequest, ColorTemperatureInteraction},
            dark_mode_group::DarkModeInteraction,
            monitor_group::{MonitorBusRequest, MonitorInteraction},
        },
        gaming_menu::{
            gamepad_group::GamepadInteraction, steam_downloads_group::SteamDownloadsInteraction,
            steam_games_group::SteamGamesInteraction,
        },
        hardware_menu::{
            compute_group::ComputeInteraction,
            gpu_group::GpuInteraction,
            memory_board_group::MemoryBoardInteraction,
            physical_drive_group::DriveInteraction,
            power_group::{PowerBusRequest, PowerInteraction},
            storage_space_group::StorageInteraction,
        },
        media_menu::{
            audio_group::AudioMessage,
            media_group::MediaInteraction,
            voice_dictation_group::{VoiceDictationBusRequest, VoiceDictationInteraction},
        },
        network_menu::{
            bluetooth_adapters_group::BluetoothInteraction,
            ethernet_ports_group::{EthernetPortsBusRequest, EthernetPortsInteraction},
            mobile_modems_group::MobileModemsInteraction,
            network_manager_vpn_group::{
                NetworkManagerVpnBusRequest, NetworkManagerVpnInteraction,
            },
            reachability_group::{ReachabilityBusRequest, ReachabilityInteraction},
            tailnet_group::{TailnetBusRequest, TailnetInteraction},
            wifi_adapters_group::{WifiAdaptersBusRequest, WifiAdaptersInteraction},
            wireguard_tunnels_group::{WireguardTunnelsBusRequest, WireguardTunnelsInteraction},
        },
        services_menu::{
            ci_runs_group::CiRunsInteraction,
            claude_usage_group::{ClaudeUsageBusRequest, ClaudeUsageInteraction},
            copilot_usage_group::{CopilotUsageBusRequest, CopilotUsageInteraction},
            journal_events_group::JournalEventsInteraction,
            llama_server_group::LlamaServerInteraction,
            openrouter_credit_group::{OpenRouterCreditBusRequest, OpenRouterCreditInteraction},
            pending_upgrades_group::PendingUpgradesInteraction,
            systemd_manager_group::SystemdManagerInteraction,
        },
        session_menu::{
            idle_pause_groups::IdlePauseInteraction,
            login_session_group::{LoginSessionBusRequest, LoginSessionInteraction},
            tray_group::TrayMessage,
        },
        session_power_menu::session_power_group::SessionMessage,
        virtualization_menu::{
            apptainer_containers_group::ApptainerContainersInteraction,
            cgroup_memory_limits_group::CgroupMemoryLimitsInteraction,
            docker_engines_group::DockerEnginesInteraction,
            flatpak_sandboxes_group::FlatpakSandboxesInteraction,
            incus_instances_group::IncusInstancesInteraction,
            lxc_containers_group::LxcContainersInteraction,
            microvm_guests_group::MicrovmGuestsInteraction,
            nspawn_containers_group::NspawnContainersInteraction,
            podman_engines_group::PodmanEnginesInteraction,
            virtual_machines_group::VirtualMachinesInteraction,
            virtualbox_machines_group::VirtualBoxMachinesInteraction,
            vmware_machines_group::VmwareMachinesInteraction,
            waydroid_container_group::WaydroidContainerInteraction,
        },
    },
};

#[derive(Debug, Clone, IntoStaticStr)]
pub enum IndicatorGroupInteraction {
    Weather(WeatherInteraction),
    Location(LocationInteraction),
    Wallpaper(IndicatorGroupExpansionMessage),
    Caldav(IndicatorGroupExpansionMessage),
    Power(PowerInteraction),
    Compute(ComputeInteraction),
    MemoryBoard(MemoryBoardInteraction),
    Gamepad(GamepadInteraction),
    Gpu(GpuInteraction),
    Storage(StorageInteraction),
    Drive(DriveInteraction),
    PanelSettings(PanelSettingsMessage),
    SidebarSettings(SidebarSettingsMessage),
    AppearancePresets(AppearancePresetsMessage),
    OverlaySettings(OverlaySettingsMessage),
    CompositorStyling(CompositorStylingMessage),
    Clock(ClockInteraction),
    NotificationGroup(NotificationGroupMessage),
    QuietSchedule(QuietScheduleMessage),
    Session(SessionMessage),
    SystemdManager(SystemdManagerInteraction),
    JournalEvents(JournalEventsInteraction),
    PendingUpgrades(PendingUpgradesInteraction),
    Reachability(ReachabilityInteraction),
    Tailnet(TailnetInteraction),
    WireguardTunnels(WireguardTunnelsInteraction),
    NetworkManagerVpns(NetworkManagerVpnInteraction),
    WifiAdapters(WifiAdaptersInteraction),
    MobileModems(MobileModemsInteraction),
    SteamDownloads(SteamDownloadsInteraction),
    SteamGames(SteamGamesInteraction),
    EthernetPorts(EthernetPortsInteraction),
    BluetoothAdapters(BluetoothInteraction),
    Monitor(MonitorInteraction),
    ColorTemperature(ColorTemperatureInteraction),
    DarkMode(DarkModeInteraction),
    VoiceDictation(VoiceDictationInteraction),
    IdleTimeouts(IdlePauseInteraction),
    Media(MediaInteraction),
    Tray(TrayMessage),
    WindowMenuBar(WindowMenuBarInteraction),
    Audio(AudioMessage),
    ClaudeUsage(ClaudeUsageInteraction),
    CopilotUsage(CopilotUsageInteraction),
    OpenRouterCredit(OpenRouterCreditInteraction),
    LlamaServer(LlamaServerInteraction),
    PodmanEngines(PodmanEnginesInteraction),
    VirtualMachines(VirtualMachinesInteraction),
    NspawnContainers(NspawnContainersInteraction),
    CgroupMemoryLimits(CgroupMemoryLimitsInteraction),
    DockerEngines(DockerEnginesInteraction),
    ApptainerContainers(ApptainerContainersInteraction),
    VmwareMachines(VmwareMachinesInteraction),
    VirtualBoxMachines(VirtualBoxMachinesInteraction),
    WaydroidContainer(WaydroidContainerInteraction),
    FlatpakSandboxes(FlatpakSandboxesInteraction),
    LxcContainers(LxcContainersInteraction),
    IncusInstances(IncusInstancesInteraction),
    MicrovmGuests(MicrovmGuestsInteraction),
    CiRuns(CiRunsInteraction),
    SessionFocus(SessionFocusMessage),
    LoginSession(LoginSessionInteraction),
    OutputWindows(OutputWindowsInteraction),
    AppLauncher(AppLauncherMessage),
    GroupMenus(GroupMenuMessage),
}

impl From<SessionMessage> for IndicatorGroupInteraction {
    fn from(interaction: SessionMessage) -> Self {
        IndicatorGroupInteraction::Session(interaction)
    }
}

impl From<SystemdManagerInteraction> for IndicatorGroupInteraction {
    fn from(interaction: SystemdManagerInteraction) -> Self {
        IndicatorGroupInteraction::SystemdManager(interaction)
    }
}

impl From<JournalEventsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: JournalEventsInteraction) -> Self {
        IndicatorGroupInteraction::JournalEvents(interaction)
    }
}

impl From<PendingUpgradesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: PendingUpgradesInteraction) -> Self {
        IndicatorGroupInteraction::PendingUpgrades(interaction)
    }
}

impl From<MonitorInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MonitorInteraction) -> Self {
        IndicatorGroupInteraction::Monitor(interaction)
    }
}

impl From<ColorTemperatureInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ColorTemperatureInteraction) -> Self {
        IndicatorGroupInteraction::ColorTemperature(interaction)
    }
}

impl From<DarkModeInteraction> for IndicatorGroupInteraction {
    fn from(interaction: DarkModeInteraction) -> Self {
        IndicatorGroupInteraction::DarkMode(interaction)
    }
}

impl From<VoiceDictationInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VoiceDictationInteraction) -> Self {
        IndicatorGroupInteraction::VoiceDictation(interaction)
    }
}

impl From<IdlePauseInteraction> for IndicatorGroupInteraction {
    fn from(interaction: IdlePauseInteraction) -> Self {
        IndicatorGroupInteraction::IdleTimeouts(interaction)
    }
}

impl From<MediaInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MediaInteraction) -> Self {
        IndicatorGroupInteraction::Media(interaction)
    }
}

impl From<TrayMessage> for IndicatorGroupInteraction {
    fn from(interaction: TrayMessage) -> Self {
        IndicatorGroupInteraction::Tray(interaction)
    }
}

impl From<WindowMenuBarInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WindowMenuBarInteraction) -> Self {
        IndicatorGroupInteraction::WindowMenuBar(interaction)
    }
}

impl From<AudioMessage> for IndicatorGroupInteraction {
    fn from(interaction: AudioMessage) -> Self {
        IndicatorGroupInteraction::Audio(interaction)
    }
}

impl From<ClaudeUsageInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ClaudeUsageInteraction) -> Self {
        IndicatorGroupInteraction::ClaudeUsage(interaction)
    }
}

impl From<CopilotUsageInteraction> for IndicatorGroupInteraction {
    fn from(interaction: CopilotUsageInteraction) -> Self {
        IndicatorGroupInteraction::CopilotUsage(interaction)
    }
}

impl From<OpenRouterCreditInteraction> for IndicatorGroupInteraction {
    fn from(interaction: OpenRouterCreditInteraction) -> Self {
        IndicatorGroupInteraction::OpenRouterCredit(interaction)
    }
}

impl From<LlamaServerInteraction> for IndicatorGroupInteraction {
    fn from(interaction: LlamaServerInteraction) -> Self {
        IndicatorGroupInteraction::LlamaServer(interaction)
    }
}

impl From<CiRunsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: CiRunsInteraction) -> Self {
        IndicatorGroupInteraction::CiRuns(interaction)
    }
}

impl From<LoginSessionInteraction> for IndicatorGroupInteraction {
    fn from(interaction: LoginSessionInteraction) -> Self {
        IndicatorGroupInteraction::LoginSession(interaction)
    }
}

impl From<OutputWindowsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: OutputWindowsInteraction) -> Self {
        IndicatorGroupInteraction::OutputWindows(interaction)
    }
}

impl From<AppLauncherMessage> for IndicatorGroupInteraction {
    fn from(interaction: AppLauncherMessage) -> Self {
        IndicatorGroupInteraction::AppLauncher(interaction)
    }
}

impl From<GroupMenuMessage> for IndicatorGroupInteraction {
    fn from(interaction: GroupMenuMessage) -> Self {
        IndicatorGroupInteraction::GroupMenus(interaction)
    }
}

#[derive(Debug, Clone)]
pub enum IndicatorGroupBusRequest {
    Weather(WeatherBusRequest),
    Power(PowerBusRequest),
    ClaudeUsage(ClaudeUsageBusRequest),
    CopilotUsage(CopilotUsageBusRequest),
    OpenRouterCredit(OpenRouterCreditBusRequest),
    LoginSession(LoginSessionBusRequest),
    VoiceDictation(VoiceDictationBusRequest),
    Reachability(ReachabilityBusRequest),
    Tailnet(TailnetBusRequest),
    WireguardTunnels(WireguardTunnelsBusRequest),
    NetworkManagerVpns(NetworkManagerVpnBusRequest),
    WifiAdapters(WifiAdaptersBusRequest),
    EthernetPorts(EthernetPortsBusRequest),
    Monitor(MonitorBusRequest),
    ColorTemperature(ColorTemperatureBusRequest),
}

impl TryFrom<IndicatorGroupInteraction> for IndicatorGroupBusRequest {
    type Error = IndicatorGroupInteraction;

    fn try_from(interaction: IndicatorGroupInteraction) -> Result<Self, Self::Error> {
        match interaction {
            IndicatorGroupInteraction::Weather(WeatherInteraction::OnBuses(request)) => {
                Ok(Self::Weather(request))
            }
            IndicatorGroupInteraction::Power(PowerInteraction::OnBuses(request)) => {
                Ok(Self::Power(request))
            }
            IndicatorGroupInteraction::ClaudeUsage(ClaudeUsageInteraction::OnBuses(request)) => {
                Ok(Self::ClaudeUsage(request))
            }
            IndicatorGroupInteraction::CopilotUsage(CopilotUsageInteraction::OnBuses(request)) => {
                Ok(Self::CopilotUsage(request))
            }
            IndicatorGroupInteraction::OpenRouterCredit(OpenRouterCreditInteraction::OnBuses(
                request,
            )) => Ok(Self::OpenRouterCredit(request)),
            IndicatorGroupInteraction::LoginSession(LoginSessionInteraction::OnBuses(request)) => {
                Ok(Self::LoginSession(request))
            }
            IndicatorGroupInteraction::VoiceDictation(VoiceDictationInteraction::OnBuses(
                request,
            )) => Ok(Self::VoiceDictation(request)),
            IndicatorGroupInteraction::Reachability(ReachabilityInteraction::OnBuses(request)) => {
                Ok(Self::Reachability(request))
            }
            IndicatorGroupInteraction::Tailnet(TailnetInteraction::OnBuses(request)) => {
                Ok(Self::Tailnet(request))
            }
            IndicatorGroupInteraction::WireguardTunnels(WireguardTunnelsInteraction::OnBuses(
                request,
            )) => Ok(Self::WireguardTunnels(request)),
            IndicatorGroupInteraction::NetworkManagerVpns(
                NetworkManagerVpnInteraction::OnBuses(request),
            ) => Ok(Self::NetworkManagerVpns(request)),
            IndicatorGroupInteraction::WifiAdapters(WifiAdaptersInteraction::OnBuses(request)) => {
                Ok(Self::WifiAdapters(request))
            }
            IndicatorGroupInteraction::EthernetPorts(EthernetPortsInteraction::OnBuses(
                request,
            )) => Ok(Self::EthernetPorts(request)),
            IndicatorGroupInteraction::Monitor(MonitorInteraction::OnBuses(request)) => {
                Ok(Self::Monitor(request))
            }
            IndicatorGroupInteraction::ColorTemperature(ColorTemperatureInteraction::OnBuses(
                request,
            )) => Ok(Self::ColorTemperature(request)),
            local_interaction => Err(local_interaction),
        }
    }
}
