use std::{collections::HashMap, sync::Arc};

use iced::window::Id;
use iced_exwlshell::{actions::RegionActionCB, to_layer_message};
use sottomano_desktop_sinks::{
    compositor_window_requests::compositor_requests::CompositorRequestFailure,
    desktop_notifications::alert_sound_player::SoundTheme,
};
use sottomano_desktop_sources::{
    compositor_window_ipc::{
        compositor_output_facts, compositor_window_state::CompositorUpdate,
        niri_kdl_config::ConfiguredWorkspaces,
    },
    delivery_cadence_drivers::ChangeWatchFailure,
    desktop_windows_apps::{
        app_launch_history::Launch, desktop_entry_app_icons::DesktopEntries,
        firefox_favicon_lookup::RawFavicon,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBusEvent,
};
use sottomano_toml_config::{Config, LoadError};
use sottomano_widgets::{
    desktop_index::ResolvedIcons,
    desktop_panels::panel_slot_drag::DragMessage,
    group_menus::group_menu_state::{GroupMenuMessage, GroupMenuPin, GroupMenuTopic},
    hover_cards::{HoverCardMessage, PopupMessage},
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widget_groups::{
        app_launcher_group::AppLauncherMessage,
        appearance_presets_group::AppearancePresetsMessage,
        apptainer_containers_group::ApptainerContainersMessage,
        audio_group::AudioMessage,
        bluetooth_adapters_group::{BluetoothInteraction, BluetoothMessage},
        caldav_group::CaldavMessage,
        cgroup_memory_limits_group::{CgroupMemoryLimitsInteraction, CgroupMemoryLimitsMessage},
        ci_runs_group::{CiRunsInteraction, CiRunsMessage},
        claude_usage_group::ClaudeUsageMessage,
        clock_group::ClockMessage,
        color_temperature_group::{ColorTemperatureInteraction, ColorTemperatureMessage},
        compositor_styling_group::CompositorStylingMessage,
        compute_group::{ComputeInteraction, ComputeMessage},
        copilot_usage_group::CopilotUsageMessage,
        dark_mode_group::{DarkModeInteraction, DarkModeMessage},
        docker_engines_group::DockerEnginesMessage,
        ethernet_ports_group::{EthernetPortsInteraction, EthernetPortsMessage},
        flatpak_sandboxes_group::FlatpakSandboxesMessage,
        gamepad_group::{GamepadInteraction, GamepadMessage},
        global_menu_group::{GlobalMenuInteraction, GlobalMenuMessage},
        gpu_group::{GpuInteraction, GpuMessage},
        idle_pause_groups::{IdlePauseInteraction, IdlePauseMessage},
        incus_instances_group::{IncusInstancesInteraction, IncusInstancesMessage},
        journal_events_group::{JournalEventsInteraction, JournalEventsMessage},
        llama_server_group::{LlamaServerInteraction, LlamaServerMessage},
        login_session_group::LoginSessionMessage,
        lxc_containers_group::LxcContainersMessage,
        media_group::{MediaInteraction, MediaMessage},
        memory_board_group::MemoryBoardMessage,
        microvm_guests_group::{MicrovmGuestsInteraction, MicrovmGuestsMessage},
        mobile_modems_group::{MobileModemsInteraction, MobileModemsMessage},
        monitor_group::{MonitorInteraction, MonitorMessage},
        network_manager_vpn_group::{NetworkManagerVpnInteraction, NetworkManagerVpnMessage},
        notification_group::NotificationGroupMessage,
        nspawn_containers_group::{NspawnContainersInteraction, NspawnContainersMessage},
        openrouter_credit_group::OpenRouterCreditMessage,
        overlay_settings_group::OverlaySettingsMessage,
        panel_settings_group::PanelSettingsMessage,
        pending_upgrades_group::{PendingUpgradesInteraction, PendingUpgradesMessage},
        physical_drive_group::{DriveInteraction, DriveMessage},
        podman_engines_group::PodmanEnginesMessage,
        power_group::{PowerInteraction, PowerMessage},
        quiet_schedule_group::QuietScheduleMessage,
        reachability_group::ReachabilityMessage,
        session_focus_group::SessionFocusMessage,
        session_power_group::SessionMessage,
        sidebar_settings_group::SidebarSettingsMessage,
        storage_space_group::{StorageInteraction, StorageMessage},
        systemd_manager_group::{SystemdManagerInteraction, SystemdManagerMessage},
        tailnet_group::{TailnetInteraction, TailnetMessage},
        tray_group::TrayMessage,
        virtual_machines_group::{VirtualMachinesInteraction, VirtualMachinesMessage},
        virtualbox_machines_group::{VirtualBoxMachinesInteraction, VirtualBoxMachinesMessage},
        vmware_machines_group::VmwareMachinesMessage,
        voice_dictation_group::{VoiceDictationInteraction, VoiceDictationMessage},
        wallpaper_group::WallpaperMessage,
        waydroid_container_group::{WaydroidContainerInteraction, WaydroidContainerMessage},
        weather_group::WeatherMessage,
        widget_group_content::{GroupEvent, WidgetGroupExpansionMessage},
        wifi_adapters_group::{WifiAdaptersInteraction, WifiAdaptersMessage},
        window_group::{WindowGroupInteraction, WindowGroupMessage},
        wireguard_tunnels_group::{WireguardTunnelsInteraction, WireguardTunnelsMessage},
        workspaces_group::WorkspacesMessage,
    },
    widgets::typing_under_cards::TypingSignal,
};

#[derive(Debug, Clone)]
pub enum OutputChange {
    Connected(String),
    Rearranged,
    Disconnected(String),
    Unavailable(String),
}

#[to_layer_message(multi)]
#[derive(Debug)]
pub enum Message {
    Weather(WeatherMessage),
    Wallpaper(WallpaperMessage),
    Caldav(CaldavMessage),
    Power(PowerMessage),
    Compute(ComputeMessage),
    MemoryBoard(MemoryBoardMessage),
    Gamepad(GamepadMessage),
    Gpu(GpuMessage),
    Storage(StorageMessage),
    Drive(DriveMessage),
    PanelSettings(PanelSettingsMessage),
    SidebarSettings(SidebarSettingsMessage),
    AppearancePresets(AppearancePresetsMessage),
    OverlaySettings(OverlaySettingsMessage),
    CompositorStyling(CompositorStylingMessage),
    Clock(ClockMessage),
    NotificationGroup(NotificationGroupMessage),
    QuietSchedule(QuietScheduleMessage),
    Session(SessionMessage),
    SystemdManager(SystemdManagerMessage),
    JournalEvents(JournalEventsMessage),
    PendingUpgrades(PendingUpgradesMessage),
    Reachability(ReachabilityMessage),
    Tailnet(TailnetMessage),
    WireguardTunnels(WireguardTunnelsMessage),
    NetworkManagerVpns(NetworkManagerVpnMessage),
    WifiAdapters(WifiAdaptersMessage),
    MobileModems(MobileModemsMessage),
    EthernetPorts(EthernetPortsMessage),
    BluetoothAdapters(BluetoothMessage),
    Monitor(MonitorMessage),
    ColorTemperature(ColorTemperatureMessage),
    DarkMode(DarkModeMessage),
    VoiceDictation(VoiceDictationMessage),
    IdleTimeouts(IdlePauseMessage),
    GroupMenuState(GroupMenuMessage),
    UnpinFromPanel {
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
    },
    Media(MediaMessage),
    Tray(TrayMessage),
    GlobalMenu(GlobalMenuMessage),
    ConfigFileChanged(Result<(), ChangeWatchFailure>),
    ConfigLoaded(Result<Box<Config>, Arc<LoadError>>),
    SoundThemeLoaded(SoundTheme),
    Audio(AudioMessage),
    ClaudeUsage(ClaudeUsageMessage),
    CopilotUsage(CopilotUsageMessage),
    OpenRouterCredit(OpenRouterCreditMessage),
    LlamaServer(LlamaServerMessage),
    PodmanEngines(PodmanEnginesMessage),
    VirtualMachines(VirtualMachinesMessage),
    NspawnContainers(NspawnContainersMessage),
    CgroupMemoryLimits(CgroupMemoryLimitsMessage),
    DockerEngines(DockerEnginesMessage),
    ApptainerContainers(ApptainerContainersMessage),
    VmwareMachines(VmwareMachinesMessage),
    VirtualBoxMachines(VirtualBoxMachinesMessage),
    WaydroidContainer(WaydroidContainerMessage),
    FlatpakSandboxes(FlatpakSandboxesMessage),
    LxcContainers(LxcContainersMessage),
    IncusInstances(IncusInstancesMessage),
    MicrovmGuests(MicrovmGuestsMessage),
    CiRuns(CiRunsMessage),
    SessionFocus(SessionFocusMessage),
    LoginSession(LoginSessionMessage),
    WindowGroup(WindowGroupMessage),
    AppLauncher(AppLauncherMessage),
    TrackerLoaded(Vec<Launch>),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    IconsResolved(ResolvedIcons),
    FaviconsResolved(HashMap<String, RawFavicon>),
    FirefoxProfileChanged(Result<(), ChangeWatchFailure>),
    DismissPressed,
    Compositor(CompositorUpdate),
    Redraw,
    WindowClosed(Id),
    PointerLeft,
    SurfacePressed(Id),
    CopyText(String),
    OpenUrl(String),
    RefreshHoveredReading,
    FocusWindow(u64),
    HoverCardHideElapsed,
    HoverCardFadeElapsed,
    TypingUnderCards(TypingSignal),
    HoverCard(HoverCardMessage),
    Notification(NotificationMessage),
    OutputsResolved(HashMap<String, compositor_output_facts::OutputInfo>),
    Output(OutputChange),
    ReloadDesktopEntries,
    SidebarWidthChanged {
        window: Id,
        width: f32,
    },
    PointerDrag(DragMessage),
    CompositorRequestFailed(CompositorRequestFailure),
    NiriConfigRead(Result<ConfiguredWorkspaces, String>),
    RestoreWorkspaceOrder {
        output: String,
    },
    DesktopBus(Result<DesktopBusEvent, zbus::Error>),
}

impl From<HoverCardMessage> for Message {
    fn from(message: HoverCardMessage) -> Self {
        Message::HoverCard(message)
    }
}

impl From<SurfaceRequest> for Message {
    fn from(surface_request: SurfaceRequest) -> Self {
        match surface_request {
            SurfaceRequest::Open { settings, surface } => Message::NewLayerShell {
                settings,
                id: surface,
            },
            SurfaceRequest::Close { surface } => Message::RemoveWindow(surface),
            SurfaceRequest::InputRegions { surface, regions } => Message::SetInputRegion {
                id: surface,
                callback: RegionActionCB::new(move |region| {
                    regions
                        .iter()
                        .filter(|rectangle| rectangle.width > 0.0 && rectangle.height > 0.0)
                        .for_each(|rectangle| {
                            let _ = region.add(
                                rectangle.x as i32,
                                rectangle.y as i32,
                                rectangle.width.ceil() as i32,
                                rectangle.height.ceil() as i32,
                            );
                        });
                }),
            },
            SurfaceRequest::KeyboardInteractivity {
                surface,
                interactivity,
            } => Message::KeyboardInteractivityChange {
                id: surface,
                keyboard_interactivity: interactivity,
            },
            SurfaceRequest::ActivationToken { surface, sink } => {
                Message::RequestActivationToken { id: surface, sink }
            }
        }
    }
}

impl From<NotificationMessage> for Message {
    fn from(notification: NotificationMessage) -> Self {
        Message::Notification(notification)
    }
}

impl From<CompositorRequestFailure> for Message {
    fn from(failure: CompositorRequestFailure) -> Self {
        Message::CompositorRequestFailed(failure)
    }
}

impl From<WorkspacesMessage> for Message {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        match workspaces_message {
            WorkspacesMessage::HoverCard(hover_card) => Message::HoverCard(hover_card),
            WorkspacesMessage::Drag(drag) => Message::PointerDrag(drag),
            WorkspacesMessage::RestoreOrder { output } => Message::RestoreWorkspaceOrder { output },
            WorkspacesMessage::ToggleEmptyWorkspaces => {
                Message::SidebarSettings(SidebarSettingsMessage::ToggleEmptyWorkspaces)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ShellInteraction {
    Weather(WidgetGroupExpansionMessage),
    Wallpaper(WidgetGroupExpansionMessage),
    Caldav(WidgetGroupExpansionMessage),
    Power(PowerInteraction),
    Compute(ComputeInteraction),
    MemoryBoard(WidgetGroupExpansionMessage),
    Gamepad(GamepadInteraction),
    Gpu(GpuInteraction),
    Storage(StorageInteraction),
    Drive(DriveInteraction),
    PanelSettings(PanelSettingsMessage),
    SidebarSettings(SidebarSettingsMessage),
    AppearancePresets(AppearancePresetsMessage),
    OverlaySettings(OverlaySettingsMessage),
    CompositorStyling(CompositorStylingMessage),
    Clock(WidgetGroupExpansionMessage),
    NotificationGroup(NotificationGroupMessage),
    QuietSchedule(QuietScheduleMessage),
    Session(SessionMessage),
    SystemdManager(SystemdManagerInteraction),
    JournalEvents(JournalEventsInteraction),
    PendingUpgrades(PendingUpgradesInteraction),
    Reachability(WidgetGroupExpansionMessage),
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
    GroupMenuState(GroupMenuMessage),
    Media(MediaInteraction),
    Tray(TrayMessage),
    GlobalMenu(GlobalMenuInteraction),
    Audio(AudioMessage),
    ClaudeUsage(WidgetGroupExpansionMessage),
    CopilotUsage(WidgetGroupExpansionMessage),
    OpenRouterCredit(WidgetGroupExpansionMessage),
    LlamaServer(LlamaServerInteraction),
    PodmanEngines(WidgetGroupExpansionMessage),
    VirtualMachines(VirtualMachinesInteraction),
    NspawnContainers(NspawnContainersInteraction),
    CgroupMemoryLimits(CgroupMemoryLimitsInteraction),
    DockerEngines(WidgetGroupExpansionMessage),
    ApptainerContainers(WidgetGroupExpansionMessage),
    VmwareMachines(WidgetGroupExpansionMessage),
    VirtualBoxMachines(VirtualBoxMachinesInteraction),
    WaydroidContainer(WaydroidContainerInteraction),
    FlatpakSandboxes(WidgetGroupExpansionMessage),
    LxcContainers(WidgetGroupExpansionMessage),
    IncusInstances(IncusInstancesInteraction),
    MicrovmGuests(MicrovmGuestsInteraction),
    CiRuns(CiRunsInteraction),
    SessionFocus(SessionFocusMessage),
    LoginSession(WidgetGroupExpansionMessage),
    WindowGroup(WindowGroupInteraction),
    AppLauncher(AppLauncherMessage),
    UnpinFromPanel {
        menu: GroupMenuTopic,
        entry: GroupMenuPin,
    },
    HoverCard(HoverCardMessage),
    Notification(NotificationMessage),
    CopyText(String),
    OpenUrl(String),
    FocusWindow(u64),
    RefreshHoveredReading,
    DismissPressed,
    SidebarWidthChanged {
        window: Id,
        width: f32,
    },
    Workspaces(WorkspacesMessage),
    SurfaceRequest(SurfaceRequest),
    CompositorRequestFailed(CompositorRequestFailure),
}

impl From<ShellInteraction> for Message {
    fn from(interaction: ShellInteraction) -> Self {
        match interaction {
            ShellInteraction::Weather(expansion) => {
                Message::Weather(WeatherMessage::Expansion(expansion))
            }
            ShellInteraction::Wallpaper(expansion) => {
                Message::Wallpaper(WallpaperMessage::Expansion(expansion))
            }
            ShellInteraction::Caldav(expansion) => {
                Message::Caldav(CaldavMessage::Expansion(expansion))
            }
            ShellInteraction::Power(interaction) => {
                Message::Power(PowerMessage::Interaction(interaction))
            }
            ShellInteraction::Compute(interaction) => {
                Message::Compute(ComputeMessage::Interaction(interaction))
            }
            ShellInteraction::MemoryBoard(expansion) => {
                Message::MemoryBoard(MemoryBoardMessage::Expansion(expansion))
            }
            ShellInteraction::Gamepad(interaction) => {
                Message::Gamepad(GamepadMessage::Interaction(interaction))
            }
            ShellInteraction::Gpu(interaction) => {
                Message::Gpu(GpuMessage::Interaction(interaction))
            }
            ShellInteraction::Storage(interaction) => {
                Message::Storage(StorageMessage::Interaction(interaction))
            }
            ShellInteraction::Drive(interaction) => {
                Message::Drive(DriveMessage::Interaction(interaction))
            }
            ShellInteraction::PanelSettings(message) => Message::PanelSettings(message),
            ShellInteraction::SidebarSettings(message) => Message::SidebarSettings(message),
            ShellInteraction::AppearancePresets(message) => Message::AppearancePresets(message),
            ShellInteraction::OverlaySettings(message) => Message::OverlaySettings(message),
            ShellInteraction::CompositorStyling(message) => Message::CompositorStyling(message),
            ShellInteraction::Clock(expansion) => {
                Message::Clock(ClockMessage::Expansion(expansion))
            }
            ShellInteraction::NotificationGroup(message) => Message::NotificationGroup(message),
            ShellInteraction::QuietSchedule(message) => Message::QuietSchedule(message),
            ShellInteraction::Session(message) => Message::Session(message),
            ShellInteraction::SystemdManager(interaction) => {
                Message::SystemdManager(SystemdManagerMessage::Interaction(interaction))
            }
            ShellInteraction::JournalEvents(interaction) => {
                Message::JournalEvents(JournalEventsMessage::Interaction(interaction))
            }
            ShellInteraction::PendingUpgrades(interaction) => {
                Message::PendingUpgrades(PendingUpgradesMessage::Interaction(interaction))
            }
            ShellInteraction::Reachability(expansion) => {
                Message::Reachability(ReachabilityMessage::Expansion(expansion))
            }
            ShellInteraction::Tailnet(interaction) => {
                Message::Tailnet(TailnetMessage::Interaction(interaction))
            }
            ShellInteraction::WireguardTunnels(interaction) => {
                Message::WireguardTunnels(WireguardTunnelsMessage::Interaction(interaction))
            }
            ShellInteraction::NetworkManagerVpns(interaction) => {
                Message::NetworkManagerVpns(NetworkManagerVpnMessage::Interaction(interaction))
            }
            ShellInteraction::WifiAdapters(interaction) => {
                Message::WifiAdapters(WifiAdaptersMessage::Interaction(interaction))
            }
            ShellInteraction::MobileModems(interaction) => {
                Message::MobileModems(MobileModemsMessage::Interaction(interaction))
            }
            ShellInteraction::EthernetPorts(interaction) => {
                Message::EthernetPorts(EthernetPortsMessage::Interaction(interaction))
            }
            ShellInteraction::BluetoothAdapters(interaction) => {
                Message::BluetoothAdapters(BluetoothMessage::Interaction(interaction))
            }
            ShellInteraction::Monitor(interaction) => {
                Message::Monitor(MonitorMessage::Interaction(interaction))
            }
            ShellInteraction::ColorTemperature(interaction) => {
                Message::ColorTemperature(ColorTemperatureMessage::Interaction(interaction))
            }
            ShellInteraction::DarkMode(interaction) => {
                Message::DarkMode(DarkModeMessage::Interaction(interaction))
            }
            ShellInteraction::VoiceDictation(interaction) => {
                Message::VoiceDictation(VoiceDictationMessage::Interaction(interaction))
            }
            ShellInteraction::IdleTimeouts(interaction) => {
                Message::IdleTimeouts(IdlePauseMessage::Interaction(interaction))
            }
            ShellInteraction::GroupMenuState(message) => Message::GroupMenuState(message),
            ShellInteraction::Media(interaction) => {
                Message::Media(MediaMessage::Interaction(interaction))
            }
            ShellInteraction::Tray(message) => Message::Tray(message),
            ShellInteraction::GlobalMenu(interaction) => {
                Message::GlobalMenu(GlobalMenuMessage::Interaction(interaction))
            }
            ShellInteraction::Audio(message) => Message::Audio(message),
            ShellInteraction::ClaudeUsage(expansion) => {
                Message::ClaudeUsage(ClaudeUsageMessage::Expansion(expansion))
            }
            ShellInteraction::CopilotUsage(expansion) => {
                Message::CopilotUsage(CopilotUsageMessage::Expansion(expansion))
            }
            ShellInteraction::OpenRouterCredit(expansion) => {
                Message::OpenRouterCredit(OpenRouterCreditMessage::Expansion(expansion))
            }
            ShellInteraction::LlamaServer(interaction) => {
                Message::LlamaServer(LlamaServerMessage::Interaction(interaction))
            }
            ShellInteraction::PodmanEngines(expansion) => {
                Message::PodmanEngines(PodmanEnginesMessage::Expansion(expansion))
            }
            ShellInteraction::VirtualMachines(interaction) => {
                Message::VirtualMachines(VirtualMachinesMessage::Interaction(interaction))
            }
            ShellInteraction::NspawnContainers(interaction) => {
                Message::NspawnContainers(NspawnContainersMessage::Interaction(interaction))
            }
            ShellInteraction::CgroupMemoryLimits(interaction) => {
                Message::CgroupMemoryLimits(CgroupMemoryLimitsMessage::Interaction(interaction))
            }
            ShellInteraction::DockerEngines(expansion) => {
                Message::DockerEngines(DockerEnginesMessage::Expansion(expansion))
            }
            ShellInteraction::ApptainerContainers(expansion) => {
                Message::ApptainerContainers(ApptainerContainersMessage::Expansion(expansion))
            }
            ShellInteraction::VmwareMachines(expansion) => {
                Message::VmwareMachines(VmwareMachinesMessage::Expansion(expansion))
            }
            ShellInteraction::VirtualBoxMachines(interaction) => {
                Message::VirtualBoxMachines(VirtualBoxMachinesMessage::Interaction(interaction))
            }
            ShellInteraction::WaydroidContainer(interaction) => {
                Message::WaydroidContainer(WaydroidContainerMessage::Interaction(interaction))
            }
            ShellInteraction::FlatpakSandboxes(expansion) => {
                Message::FlatpakSandboxes(FlatpakSandboxesMessage::Expansion(expansion))
            }
            ShellInteraction::LxcContainers(expansion) => {
                Message::LxcContainers(LxcContainersMessage::Expansion(expansion))
            }
            ShellInteraction::IncusInstances(interaction) => {
                Message::IncusInstances(IncusInstancesMessage::Interaction(interaction))
            }
            ShellInteraction::MicrovmGuests(interaction) => {
                Message::MicrovmGuests(MicrovmGuestsMessage::Interaction(interaction))
            }
            ShellInteraction::CiRuns(interaction) => {
                Message::CiRuns(CiRunsMessage::Interaction(interaction))
            }
            ShellInteraction::SessionFocus(message) => Message::SessionFocus(message),
            ShellInteraction::LoginSession(expansion) => {
                Message::LoginSession(LoginSessionMessage::Expansion(expansion))
            }
            ShellInteraction::WindowGroup(interaction) => {
                Message::WindowGroup(WindowGroupMessage::Interaction(interaction))
            }
            ShellInteraction::AppLauncher(message) => Message::AppLauncher(message),
            ShellInteraction::UnpinFromPanel { menu, entry } => {
                Message::UnpinFromPanel { menu, entry }
            }
            ShellInteraction::HoverCard(message) => Message::HoverCard(message),
            ShellInteraction::Notification(message) => Message::Notification(message),
            ShellInteraction::CopyText(copied) => Message::CopyText(copied),
            ShellInteraction::OpenUrl(url) => Message::OpenUrl(url),
            ShellInteraction::FocusWindow(window) => Message::FocusWindow(window),
            ShellInteraction::RefreshHoveredReading => Message::RefreshHoveredReading,
            ShellInteraction::DismissPressed => Message::DismissPressed,
            ShellInteraction::SidebarWidthChanged { window, width } => {
                Message::SidebarWidthChanged { window, width }
            }
            ShellInteraction::Workspaces(workspaces_message) => Message::from(workspaces_message),
            ShellInteraction::SurfaceRequest(surface_request) => Message::from(surface_request),
            ShellInteraction::CompositorRequestFailed(failure) => {
                Message::CompositorRequestFailed(failure)
            }
        }
    }
}

impl From<WorkspacesMessage> for ShellInteraction {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        ShellInteraction::Workspaces(workspaces_message)
    }
}

impl From<SurfaceRequest> for ShellInteraction {
    fn from(surface_request: SurfaceRequest) -> Self {
        ShellInteraction::SurfaceRequest(surface_request)
    }
}

impl From<NotificationMessage> for ShellInteraction {
    fn from(notification: NotificationMessage) -> Self {
        ShellInteraction::Notification(notification)
    }
}

impl From<CompositorRequestFailure> for ShellInteraction {
    fn from(failure: CompositorRequestFailure) -> Self {
        ShellInteraction::CompositorRequestFailed(failure)
    }
}

impl From<HoverCardMessage> for ShellInteraction {
    fn from(message: HoverCardMessage) -> Self {
        ShellInteraction::HoverCard(message)
    }
}

impl From<GroupEvent<ShellInteraction>> for ShellInteraction {
    fn from(event: GroupEvent<ShellInteraction>) -> Self {
        match event {
            GroupEvent::Group(interaction) => interaction,
            GroupEvent::HoverCard(hover_card) => ShellInteraction::HoverCard(hover_card),
        }
    }
}

impl From<PopupMessage> for ShellInteraction {
    fn from(popup_message: PopupMessage) -> Self {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => ShellInteraction::HoverCard(hover_card),
            PopupMessage::CopyText(copied) => ShellInteraction::CopyText(copied),
            PopupMessage::OpenUrl(url) => ShellInteraction::OpenUrl(url),
            PopupMessage::FocusWindow(window) => ShellInteraction::FocusWindow(window),
            PopupMessage::RefreshHoveredReading => ShellInteraction::RefreshHoveredReading,
            PopupMessage::WindowGroup(window_group) => ShellInteraction::WindowGroup(window_group),
            PopupMessage::ToggleGroupMenuPin { menu, entry } => {
                ShellInteraction::UnpinFromPanel { menu, entry }
            }
        }
    }
}
