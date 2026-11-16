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

impl From<WeatherInteraction> for WidgetGroupInteraction {
    fn from(interaction: WeatherInteraction) -> Self {
        WidgetGroupInteraction::Weather(interaction)
    }
}

impl From<PowerInteraction> for WidgetGroupInteraction {
    fn from(interaction: PowerInteraction) -> Self {
        WidgetGroupInteraction::Power(interaction)
    }
}

impl From<ComputeInteraction> for WidgetGroupInteraction {
    fn from(interaction: ComputeInteraction) -> Self {
        WidgetGroupInteraction::Compute(interaction)
    }
}

impl From<MemoryBoardInteraction> for WidgetGroupInteraction {
    fn from(interaction: MemoryBoardInteraction) -> Self {
        WidgetGroupInteraction::MemoryBoard(interaction)
    }
}

impl From<GamepadInteraction> for WidgetGroupInteraction {
    fn from(interaction: GamepadInteraction) -> Self {
        WidgetGroupInteraction::Gamepad(interaction)
    }
}

impl From<GpuInteraction> for WidgetGroupInteraction {
    fn from(interaction: GpuInteraction) -> Self {
        WidgetGroupInteraction::Gpu(interaction)
    }
}

impl From<StorageInteraction> for WidgetGroupInteraction {
    fn from(interaction: StorageInteraction) -> Self {
        WidgetGroupInteraction::Storage(interaction)
    }
}

impl From<DriveInteraction> for WidgetGroupInteraction {
    fn from(interaction: DriveInteraction) -> Self {
        WidgetGroupInteraction::Drive(interaction)
    }
}

impl From<PanelSettingsMessage> for WidgetGroupInteraction {
    fn from(interaction: PanelSettingsMessage) -> Self {
        WidgetGroupInteraction::PanelSettings(interaction)
    }
}

impl From<SidebarSettingsMessage> for WidgetGroupInteraction {
    fn from(interaction: SidebarSettingsMessage) -> Self {
        WidgetGroupInteraction::SidebarSettings(interaction)
    }
}

impl From<AppearancePresetsMessage> for WidgetGroupInteraction {
    fn from(interaction: AppearancePresetsMessage) -> Self {
        WidgetGroupInteraction::AppearancePresets(interaction)
    }
}

impl From<OverlaySettingsMessage> for WidgetGroupInteraction {
    fn from(interaction: OverlaySettingsMessage) -> Self {
        WidgetGroupInteraction::OverlaySettings(interaction)
    }
}

impl From<CompositorStylingMessage> for WidgetGroupInteraction {
    fn from(interaction: CompositorStylingMessage) -> Self {
        WidgetGroupInteraction::CompositorStyling(interaction)
    }
}

impl From<ClockInteraction> for WidgetGroupInteraction {
    fn from(interaction: ClockInteraction) -> Self {
        WidgetGroupInteraction::Clock(interaction)
    }
}

impl From<NotificationGroupMessage> for WidgetGroupInteraction {
    fn from(interaction: NotificationGroupMessage) -> Self {
        WidgetGroupInteraction::NotificationGroup(interaction)
    }
}

impl From<QuietScheduleMessage> for WidgetGroupInteraction {
    fn from(interaction: QuietScheduleMessage) -> Self {
        WidgetGroupInteraction::QuietSchedule(interaction)
    }
}

impl From<SessionMessage> for WidgetGroupInteraction {
    fn from(interaction: SessionMessage) -> Self {
        WidgetGroupInteraction::Session(interaction)
    }
}

impl From<SystemdManagerInteraction> for WidgetGroupInteraction {
    fn from(interaction: SystemdManagerInteraction) -> Self {
        WidgetGroupInteraction::SystemdManager(interaction)
    }
}

impl From<JournalEventsInteraction> for WidgetGroupInteraction {
    fn from(interaction: JournalEventsInteraction) -> Self {
        WidgetGroupInteraction::JournalEvents(interaction)
    }
}

impl From<PendingUpgradesInteraction> for WidgetGroupInteraction {
    fn from(interaction: PendingUpgradesInteraction) -> Self {
        WidgetGroupInteraction::PendingUpgrades(interaction)
    }
}

impl From<ReachabilityInteraction> for WidgetGroupInteraction {
    fn from(interaction: ReachabilityInteraction) -> Self {
        WidgetGroupInteraction::Reachability(interaction)
    }
}

impl From<TailnetInteraction> for WidgetGroupInteraction {
    fn from(interaction: TailnetInteraction) -> Self {
        WidgetGroupInteraction::Tailnet(interaction)
    }
}

impl From<WireguardTunnelsInteraction> for WidgetGroupInteraction {
    fn from(interaction: WireguardTunnelsInteraction) -> Self {
        WidgetGroupInteraction::WireguardTunnels(interaction)
    }
}

impl From<NetworkManagerVpnInteraction> for WidgetGroupInteraction {
    fn from(interaction: NetworkManagerVpnInteraction) -> Self {
        WidgetGroupInteraction::NetworkManagerVpns(interaction)
    }
}

impl From<WifiAdaptersInteraction> for WidgetGroupInteraction {
    fn from(interaction: WifiAdaptersInteraction) -> Self {
        WidgetGroupInteraction::WifiAdapters(interaction)
    }
}

impl From<MobileModemsInteraction> for WidgetGroupInteraction {
    fn from(interaction: MobileModemsInteraction) -> Self {
        WidgetGroupInteraction::MobileModems(interaction)
    }
}

impl From<EthernetPortsInteraction> for WidgetGroupInteraction {
    fn from(interaction: EthernetPortsInteraction) -> Self {
        WidgetGroupInteraction::EthernetPorts(interaction)
    }
}

impl From<BluetoothInteraction> for WidgetGroupInteraction {
    fn from(interaction: BluetoothInteraction) -> Self {
        WidgetGroupInteraction::BluetoothAdapters(interaction)
    }
}

impl From<MonitorInteraction> for WidgetGroupInteraction {
    fn from(interaction: MonitorInteraction) -> Self {
        WidgetGroupInteraction::Monitor(interaction)
    }
}

impl From<ColorTemperatureInteraction> for WidgetGroupInteraction {
    fn from(interaction: ColorTemperatureInteraction) -> Self {
        WidgetGroupInteraction::ColorTemperature(interaction)
    }
}

impl From<DarkModeInteraction> for WidgetGroupInteraction {
    fn from(interaction: DarkModeInteraction) -> Self {
        WidgetGroupInteraction::DarkMode(interaction)
    }
}

impl From<VoiceDictationInteraction> for WidgetGroupInteraction {
    fn from(interaction: VoiceDictationInteraction) -> Self {
        WidgetGroupInteraction::VoiceDictation(interaction)
    }
}

impl From<IdlePauseInteraction> for WidgetGroupInteraction {
    fn from(interaction: IdlePauseInteraction) -> Self {
        WidgetGroupInteraction::IdleTimeouts(interaction)
    }
}

impl From<MediaInteraction> for WidgetGroupInteraction {
    fn from(interaction: MediaInteraction) -> Self {
        WidgetGroupInteraction::Media(interaction)
    }
}

impl From<TrayMessage> for WidgetGroupInteraction {
    fn from(interaction: TrayMessage) -> Self {
        WidgetGroupInteraction::Tray(interaction)
    }
}

impl From<GlobalMenuInteraction> for WidgetGroupInteraction {
    fn from(interaction: GlobalMenuInteraction) -> Self {
        WidgetGroupInteraction::GlobalMenu(interaction)
    }
}

impl From<AudioMessage> for WidgetGroupInteraction {
    fn from(interaction: AudioMessage) -> Self {
        WidgetGroupInteraction::Audio(interaction)
    }
}

impl From<ClaudeUsageInteraction> for WidgetGroupInteraction {
    fn from(interaction: ClaudeUsageInteraction) -> Self {
        WidgetGroupInteraction::ClaudeUsage(interaction)
    }
}

impl From<CopilotUsageInteraction> for WidgetGroupInteraction {
    fn from(interaction: CopilotUsageInteraction) -> Self {
        WidgetGroupInteraction::CopilotUsage(interaction)
    }
}

impl From<OpenRouterCreditInteraction> for WidgetGroupInteraction {
    fn from(interaction: OpenRouterCreditInteraction) -> Self {
        WidgetGroupInteraction::OpenRouterCredit(interaction)
    }
}

impl From<LlamaServerInteraction> for WidgetGroupInteraction {
    fn from(interaction: LlamaServerInteraction) -> Self {
        WidgetGroupInteraction::LlamaServer(interaction)
    }
}

impl From<VirtualMachinesInteraction> for WidgetGroupInteraction {
    fn from(interaction: VirtualMachinesInteraction) -> Self {
        WidgetGroupInteraction::VirtualMachines(interaction)
    }
}

impl From<NspawnContainersInteraction> for WidgetGroupInteraction {
    fn from(interaction: NspawnContainersInteraction) -> Self {
        WidgetGroupInteraction::NspawnContainers(interaction)
    }
}

impl From<CgroupMemoryLimitsInteraction> for WidgetGroupInteraction {
    fn from(interaction: CgroupMemoryLimitsInteraction) -> Self {
        WidgetGroupInteraction::CgroupMemoryLimits(interaction)
    }
}

impl From<ApptainerContainersInteraction> for WidgetGroupInteraction {
    fn from(interaction: ApptainerContainersInteraction) -> Self {
        WidgetGroupInteraction::ApptainerContainers(interaction)
    }
}

impl From<VmwareMachinesInteraction> for WidgetGroupInteraction {
    fn from(interaction: VmwareMachinesInteraction) -> Self {
        WidgetGroupInteraction::VmwareMachines(interaction)
    }
}

impl From<VirtualBoxMachinesInteraction> for WidgetGroupInteraction {
    fn from(interaction: VirtualBoxMachinesInteraction) -> Self {
        WidgetGroupInteraction::VirtualBoxMachines(interaction)
    }
}

impl From<WaydroidContainerInteraction> for WidgetGroupInteraction {
    fn from(interaction: WaydroidContainerInteraction) -> Self {
        WidgetGroupInteraction::WaydroidContainer(interaction)
    }
}

impl From<FlatpakSandboxesInteraction> for WidgetGroupInteraction {
    fn from(interaction: FlatpakSandboxesInteraction) -> Self {
        WidgetGroupInteraction::FlatpakSandboxes(interaction)
    }
}

impl From<LxcContainersInteraction> for WidgetGroupInteraction {
    fn from(interaction: LxcContainersInteraction) -> Self {
        WidgetGroupInteraction::LxcContainers(interaction)
    }
}

impl From<IncusInstancesInteraction> for WidgetGroupInteraction {
    fn from(interaction: IncusInstancesInteraction) -> Self {
        WidgetGroupInteraction::IncusInstances(interaction)
    }
}

impl From<MicrovmGuestsInteraction> for WidgetGroupInteraction {
    fn from(interaction: MicrovmGuestsInteraction) -> Self {
        WidgetGroupInteraction::MicrovmGuests(interaction)
    }
}

impl From<CiRunsInteraction> for WidgetGroupInteraction {
    fn from(interaction: CiRunsInteraction) -> Self {
        WidgetGroupInteraction::CiRuns(interaction)
    }
}

impl From<SessionFocusMessage> for WidgetGroupInteraction {
    fn from(interaction: SessionFocusMessage) -> Self {
        WidgetGroupInteraction::SessionFocus(interaction)
    }
}

impl From<LoginSessionInteraction> for WidgetGroupInteraction {
    fn from(interaction: LoginSessionInteraction) -> Self {
        WidgetGroupInteraction::LoginSession(interaction)
    }
}

impl From<WindowGroupInteraction> for WidgetGroupInteraction {
    fn from(interaction: WindowGroupInteraction) -> Self {
        WidgetGroupInteraction::WindowGroup(interaction)
    }
}

impl From<AppLauncherMessage> for WidgetGroupInteraction {
    fn from(interaction: AppLauncherMessage) -> Self {
        WidgetGroupInteraction::AppLauncher(interaction)
    }
}

impl From<GroupMenuMessage> for WidgetGroupInteraction {
    fn from(interaction: GroupMenuMessage) -> Self {
        WidgetGroupInteraction::GroupMenus(interaction)
    }
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
