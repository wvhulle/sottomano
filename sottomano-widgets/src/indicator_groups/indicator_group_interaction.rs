use strum::IntoStaticStr;

use crate::{
    cascading_menus::{
        app_launcher_menu::AppLauncherMessage, window_menu_bar::WindowMenuBarInteraction,
    },
    group_menus::group_menu_state::GroupMenuMessage,
    indicator_groups::{
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
        gpu_group::GpuInteraction,
        idle_pause_groups::IdlePauseInteraction,
        incus_instances_group::IncusInstancesInteraction,
        indicator_group_content::IndicatorGroupExpansionMessage,
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
        wifi_adapters_group::{WifiAdaptersBusRequest, WifiAdaptersInteraction},
        window_group::WindowGroupInteraction,
        wireguard_tunnels_group::{WireguardTunnelsBusRequest, WireguardTunnelsInteraction},
    },
};

#[derive(Debug, Clone, IntoStaticStr)]
pub enum IndicatorGroupInteraction {
    Weather(WeatherInteraction),
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
    WindowGroup(WindowGroupInteraction),
    AppLauncher(AppLauncherMessage),
    GroupMenus(GroupMenuMessage),
}

impl From<WeatherInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WeatherInteraction) -> Self {
        IndicatorGroupInteraction::Weather(interaction)
    }
}

impl From<PowerInteraction> for IndicatorGroupInteraction {
    fn from(interaction: PowerInteraction) -> Self {
        IndicatorGroupInteraction::Power(interaction)
    }
}

impl From<ComputeInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ComputeInteraction) -> Self {
        IndicatorGroupInteraction::Compute(interaction)
    }
}

impl From<MemoryBoardInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MemoryBoardInteraction) -> Self {
        IndicatorGroupInteraction::MemoryBoard(interaction)
    }
}

impl From<GamepadInteraction> for IndicatorGroupInteraction {
    fn from(interaction: GamepadInteraction) -> Self {
        IndicatorGroupInteraction::Gamepad(interaction)
    }
}

impl From<GpuInteraction> for IndicatorGroupInteraction {
    fn from(interaction: GpuInteraction) -> Self {
        IndicatorGroupInteraction::Gpu(interaction)
    }
}

impl From<StorageInteraction> for IndicatorGroupInteraction {
    fn from(interaction: StorageInteraction) -> Self {
        IndicatorGroupInteraction::Storage(interaction)
    }
}

impl From<DriveInteraction> for IndicatorGroupInteraction {
    fn from(interaction: DriveInteraction) -> Self {
        IndicatorGroupInteraction::Drive(interaction)
    }
}

impl From<PanelSettingsMessage> for IndicatorGroupInteraction {
    fn from(interaction: PanelSettingsMessage) -> Self {
        IndicatorGroupInteraction::PanelSettings(interaction)
    }
}

impl From<SidebarSettingsMessage> for IndicatorGroupInteraction {
    fn from(interaction: SidebarSettingsMessage) -> Self {
        IndicatorGroupInteraction::SidebarSettings(interaction)
    }
}

impl From<AppearancePresetsMessage> for IndicatorGroupInteraction {
    fn from(interaction: AppearancePresetsMessage) -> Self {
        IndicatorGroupInteraction::AppearancePresets(interaction)
    }
}

impl From<OverlaySettingsMessage> for IndicatorGroupInteraction {
    fn from(interaction: OverlaySettingsMessage) -> Self {
        IndicatorGroupInteraction::OverlaySettings(interaction)
    }
}

impl From<CompositorStylingMessage> for IndicatorGroupInteraction {
    fn from(interaction: CompositorStylingMessage) -> Self {
        IndicatorGroupInteraction::CompositorStyling(interaction)
    }
}

impl From<ClockInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ClockInteraction) -> Self {
        IndicatorGroupInteraction::Clock(interaction)
    }
}

impl From<NotificationGroupMessage> for IndicatorGroupInteraction {
    fn from(interaction: NotificationGroupMessage) -> Self {
        IndicatorGroupInteraction::NotificationGroup(interaction)
    }
}

impl From<QuietScheduleMessage> for IndicatorGroupInteraction {
    fn from(interaction: QuietScheduleMessage) -> Self {
        IndicatorGroupInteraction::QuietSchedule(interaction)
    }
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

impl From<ReachabilityInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ReachabilityInteraction) -> Self {
        IndicatorGroupInteraction::Reachability(interaction)
    }
}

impl From<TailnetInteraction> for IndicatorGroupInteraction {
    fn from(interaction: TailnetInteraction) -> Self {
        IndicatorGroupInteraction::Tailnet(interaction)
    }
}

impl From<WireguardTunnelsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WireguardTunnelsInteraction) -> Self {
        IndicatorGroupInteraction::WireguardTunnels(interaction)
    }
}

impl From<NetworkManagerVpnInteraction> for IndicatorGroupInteraction {
    fn from(interaction: NetworkManagerVpnInteraction) -> Self {
        IndicatorGroupInteraction::NetworkManagerVpns(interaction)
    }
}

impl From<WifiAdaptersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WifiAdaptersInteraction) -> Self {
        IndicatorGroupInteraction::WifiAdapters(interaction)
    }
}

impl From<MobileModemsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MobileModemsInteraction) -> Self {
        IndicatorGroupInteraction::MobileModems(interaction)
    }
}

impl From<EthernetPortsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: EthernetPortsInteraction) -> Self {
        IndicatorGroupInteraction::EthernetPorts(interaction)
    }
}

impl From<BluetoothInteraction> for IndicatorGroupInteraction {
    fn from(interaction: BluetoothInteraction) -> Self {
        IndicatorGroupInteraction::BluetoothAdapters(interaction)
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

impl From<VirtualMachinesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VirtualMachinesInteraction) -> Self {
        IndicatorGroupInteraction::VirtualMachines(interaction)
    }
}

impl From<NspawnContainersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: NspawnContainersInteraction) -> Self {
        IndicatorGroupInteraction::NspawnContainers(interaction)
    }
}

impl From<CgroupMemoryLimitsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: CgroupMemoryLimitsInteraction) -> Self {
        IndicatorGroupInteraction::CgroupMemoryLimits(interaction)
    }
}

impl From<ApptainerContainersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ApptainerContainersInteraction) -> Self {
        IndicatorGroupInteraction::ApptainerContainers(interaction)
    }
}

impl From<VmwareMachinesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VmwareMachinesInteraction) -> Self {
        IndicatorGroupInteraction::VmwareMachines(interaction)
    }
}

impl From<VirtualBoxMachinesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: VirtualBoxMachinesInteraction) -> Self {
        IndicatorGroupInteraction::VirtualBoxMachines(interaction)
    }
}

impl From<WaydroidContainerInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WaydroidContainerInteraction) -> Self {
        IndicatorGroupInteraction::WaydroidContainer(interaction)
    }
}

impl From<FlatpakSandboxesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: FlatpakSandboxesInteraction) -> Self {
        IndicatorGroupInteraction::FlatpakSandboxes(interaction)
    }
}

impl From<LxcContainersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: LxcContainersInteraction) -> Self {
        IndicatorGroupInteraction::LxcContainers(interaction)
    }
}

impl From<IncusInstancesInteraction> for IndicatorGroupInteraction {
    fn from(interaction: IncusInstancesInteraction) -> Self {
        IndicatorGroupInteraction::IncusInstances(interaction)
    }
}

impl From<MicrovmGuestsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MicrovmGuestsInteraction) -> Self {
        IndicatorGroupInteraction::MicrovmGuests(interaction)
    }
}

impl From<CiRunsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: CiRunsInteraction) -> Self {
        IndicatorGroupInteraction::CiRuns(interaction)
    }
}

impl From<SessionFocusMessage> for IndicatorGroupInteraction {
    fn from(interaction: SessionFocusMessage) -> Self {
        IndicatorGroupInteraction::SessionFocus(interaction)
    }
}

impl From<LoginSessionInteraction> for IndicatorGroupInteraction {
    fn from(interaction: LoginSessionInteraction) -> Self {
        IndicatorGroupInteraction::LoginSession(interaction)
    }
}

impl From<WindowGroupInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WindowGroupInteraction) -> Self {
        IndicatorGroupInteraction::WindowGroup(interaction)
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
