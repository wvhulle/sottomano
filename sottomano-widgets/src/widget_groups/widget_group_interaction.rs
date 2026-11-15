use crate::{
    group_menus::group_menu_state::GroupMenuMessage,
    widget_groups::{
        app_launcher_group::AppLauncherMessage,
        appearance_presets_group::AppearancePresetsMessage,
        apptainer_containers_group::ApptainerContainersInteraction,
        audio_group::AudioMessage,
        bluetooth_adapters_group::BluetoothInteraction,
        cgroup_memory_limits_group::CgroupMemoryLimitsInteraction,
        ci_runs_group::CiRunsInteraction,
        claude_usage_group::{ClaudeUsageBusRequest, ClaudeUsageInteraction},
        clock_group::ClockInteraction,
        color_temperature_group::{ColorTemperatureBusRequest, ColorTemperatureInteraction},
        compositor_styling_group::CompositorStylingMessage,
        compute_group::ComputeInteraction,
        copilot_usage_group::{CopilotUsageBusRequest, CopilotUsageInteraction},
        dark_mode_group::DarkModeInteraction,
        docker_engines_group::DockerEnginesInteraction,
        ethernet_ports_group::{EthernetPortsBusRequest, EthernetPortsInteraction},
        flatpak_sandboxes_group::FlatpakSandboxesInteraction,
        gamepad_group::GamepadInteraction,
        global_menu_group::GlobalMenuInteraction,
        gpu_group::GpuInteraction,
        idle_pause_groups::IdlePauseInteraction,
        incus_instances_group::IncusInstancesInteraction,
        journal_events_group::JournalEventsInteraction,
        llama_server_group::LlamaServerInteraction,
        login_session_group::{LoginSessionBusRequest, LoginSessionInteraction},
        lxc_containers_group::LxcContainersInteraction,
        media_group::MediaInteraction,
        memory_board_group::MemoryBoardInteraction,
        microvm_guests_group::MicrovmGuestsInteraction,
        mobile_modems_group::MobileModemsInteraction,
        monitor_group::{MonitorBusRequest, MonitorInteraction},
        network_manager_vpn_group::{NetworkManagerVpnBusRequest, NetworkManagerVpnInteraction},
        notification_group::NotificationGroupMessage,
        nspawn_containers_group::NspawnContainersInteraction,
        openrouter_credit_group::{OpenRouterCreditBusRequest, OpenRouterCreditInteraction},
        overlay_settings_group::OverlaySettingsMessage,
        panel_settings_group::PanelSettingsMessage,
        pending_upgrades_group::PendingUpgradesInteraction,
        physical_drive_group::DriveInteraction,
        podman_engines_group::PodmanEnginesInteraction,
        power_group::{PowerBusRequest, PowerInteraction},
        quiet_schedule_group::QuietScheduleMessage,
        reachability_group::{ReachabilityBusRequest, ReachabilityInteraction},
        session_focus_group::SessionFocusMessage,
        session_power_group::SessionMessage,
        sidebar_settings_group::SidebarSettingsMessage,
        storage_space_group::StorageInteraction,
        systemd_manager_group::SystemdManagerInteraction,
        tailnet_group::{TailnetBusRequest, TailnetInteraction},
        tray_group::TrayMessage,
        virtual_machines_group::VirtualMachinesInteraction,
        virtualbox_machines_group::VirtualBoxMachinesInteraction,
        vmware_machines_group::VmwareMachinesInteraction,
        voice_dictation_group::{VoiceDictationBusRequest, VoiceDictationInteraction},
        waydroid_container_group::WaydroidContainerInteraction,
        weather_group::{WeatherBusRequest, WeatherInteraction},
        widget_group_content::WidgetGroupExpansionMessage,
        wifi_adapters_group::{WifiAdaptersBusRequest, WifiAdaptersInteraction},
        window_group::WindowGroupInteraction,
        wireguard_tunnels_group::{WireguardTunnelsBusRequest, WireguardTunnelsInteraction},
    },
};

#[derive(Debug, Clone)]
pub enum WidgetGroupInteraction {
    Weather(WeatherInteraction),
    Wallpaper(WidgetGroupExpansionMessage),
    Caldav(WidgetGroupExpansionMessage),
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
    EthernetPorts(EthernetPortsInteraction),
    BluetoothAdapters(BluetoothInteraction),
    Monitor(MonitorInteraction),
    ColorTemperature(ColorTemperatureInteraction),
    DarkMode(DarkModeInteraction),
    VoiceDictation(VoiceDictationInteraction),
    IdleTimeouts(IdlePauseInteraction),
    Media(MediaInteraction),
    Tray(TrayMessage),
    GlobalMenu(GlobalMenuInteraction),
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
    WindowGroup(WindowGroupInteraction),
    AppLauncher(AppLauncherMessage),
    GroupMenus(GroupMenuMessage),
}

#[derive(Debug, Clone)]
pub enum WidgetGroupBusRequest {
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

impl TryFrom<WidgetGroupInteraction> for WidgetGroupBusRequest {
    type Error = WidgetGroupInteraction;

    fn try_from(interaction: WidgetGroupInteraction) -> Result<Self, Self::Error> {
        match interaction {
            WidgetGroupInteraction::Weather(WeatherInteraction::OnBuses(request)) => {
                Ok(Self::Weather(request))
            }
            WidgetGroupInteraction::Power(PowerInteraction::OnBuses(request)) => {
                Ok(Self::Power(request))
            }
            WidgetGroupInteraction::ClaudeUsage(ClaudeUsageInteraction::OnBuses(request)) => {
                Ok(Self::ClaudeUsage(request))
            }
            WidgetGroupInteraction::CopilotUsage(CopilotUsageInteraction::OnBuses(request)) => {
                Ok(Self::CopilotUsage(request))
            }
            WidgetGroupInteraction::OpenRouterCredit(OpenRouterCreditInteraction::OnBuses(
                request,
            )) => Ok(Self::OpenRouterCredit(request)),
            WidgetGroupInteraction::LoginSession(LoginSessionInteraction::OnBuses(request)) => {
                Ok(Self::LoginSession(request))
            }
            WidgetGroupInteraction::VoiceDictation(VoiceDictationInteraction::OnBuses(request)) => {
                Ok(Self::VoiceDictation(request))
            }
            WidgetGroupInteraction::Reachability(ReachabilityInteraction::OnBuses(request)) => {
                Ok(Self::Reachability(request))
            }
            WidgetGroupInteraction::Tailnet(TailnetInteraction::OnBuses(request)) => {
                Ok(Self::Tailnet(request))
            }
            WidgetGroupInteraction::WireguardTunnels(WireguardTunnelsInteraction::OnBuses(
                request,
            )) => Ok(Self::WireguardTunnels(request)),
            WidgetGroupInteraction::NetworkManagerVpns(NetworkManagerVpnInteraction::OnBuses(
                request,
            )) => Ok(Self::NetworkManagerVpns(request)),
            WidgetGroupInteraction::WifiAdapters(WifiAdaptersInteraction::OnBuses(request)) => {
                Ok(Self::WifiAdapters(request))
            }
            WidgetGroupInteraction::EthernetPorts(EthernetPortsInteraction::OnBuses(request)) => {
                Ok(Self::EthernetPorts(request))
            }
            WidgetGroupInteraction::Monitor(MonitorInteraction::OnBuses(request)) => {
                Ok(Self::Monitor(request))
            }
            WidgetGroupInteraction::ColorTemperature(ColorTemperatureInteraction::OnBuses(
                request,
            )) => Ok(Self::ColorTemperature(request)),
            local_interaction => Err(local_interaction),
        }
    }
}
