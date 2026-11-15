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
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{HoverCardMessage, PopupMessage},
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widget_groups::{
        app_launcher_group::AppLauncherMessage,
        apptainer_containers_group::ApptainerContainersMessage, audio_group::AudioMessage,
        bluetooth_adapters_group::BluetoothMessage, caldav_group::CaldavMessage,
        cgroup_memory_limits_group::CgroupMemoryLimitsMessage, ci_runs_group::CiRunsMessage,
        claude_usage_group::ClaudeUsageMessage, clock_group::ClockMessage,
        color_temperature_group::ColorTemperatureMessage,
        compositor_styling_group::CompositorStylingMessage, compute_group::ComputeMessage,
        copilot_usage_group::CopilotUsageMessage, dark_mode_group::DarkModeMessage,
        docker_engines_group::DockerEnginesMessage, ethernet_ports_group::EthernetPortsMessage,
        flatpak_sandboxes_group::FlatpakSandboxesMessage, gamepad_group::GamepadMessage,
        global_menu_group::GlobalMenuMessage, gpu_group::GpuMessage,
        idle_pause_groups::IdlePauseMessage, incus_instances_group::IncusInstancesMessage,
        journal_events_group::JournalEventsMessage, llama_server_group::LlamaServerMessage,
        login_session_group::LoginSessionMessage, lxc_containers_group::LxcContainersMessage,
        media_group::MediaMessage, memory_board_group::MemoryBoardMessage,
        microvm_guests_group::MicrovmGuestsMessage, mobile_modems_group::MobileModemsMessage,
        monitor_group::MonitorMessage, network_manager_vpn_group::NetworkManagerVpnMessage,
        notification_group::NotificationGroupMessage,
        nspawn_containers_group::NspawnContainersMessage,
        openrouter_credit_group::OpenRouterCreditMessage,
        overlay_settings_group::OverlaySettingsMessage, panel_settings_group::PanelSettingsMessage,
        pending_upgrades_group::PendingUpgradesMessage, physical_drive_group::DriveMessage,
        podman_engines_group::PodmanEnginesMessage, power_group::PowerMessage,
        quiet_schedule_group::QuietScheduleMessage, reachability_group::ReachabilityMessage,
        session_focus_group::SessionFocusMessage, session_power_group::SessionMessage,
        sidebar_settings_group::SidebarSettingsMessage, storage_space_group::StorageMessage,
        systemd_manager_group::SystemdManagerMessage, tailnet_group::TailnetMessage,
        tray_group::TrayMessage, virtual_machines_group::VirtualMachinesMessage,
        virtualbox_machines_group::VirtualBoxMachinesMessage,
        vmware_machines_group::VmwareMachinesMessage, voice_dictation_group::VoiceDictationMessage,
        wallpaper_group::WallpaperMessage, waydroid_container_group::WaydroidContainerMessage,
        weather_group::WeatherMessage, widget_group_content::GroupEvent,
        widget_group_interaction::WidgetGroupInteraction, wifi_adapters_group::WifiAdaptersMessage,
        window_group::WindowGroupMessage, wireguard_tunnels_group::WireguardTunnelsMessage,
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
    Compositor(CompositorUpdate),
    Redraw,
    WindowClosed(Id),
    PointerLeft,
    SurfacePressed(Id),
    HoverCardHideElapsed,
    HoverCardFadeElapsed,
    TypingUnderCards(TypingSignal),
    Notification(NotificationMessage),
    NotificationServerFailed(zbus::Error),
    OutputsResolved(HashMap<String, compositor_output_facts::OutputInfo>),
    Output(OutputChange),
    ReloadDesktopEntries,
    PointerDrag(DragMessage),
    CompositorRequestFailed(CompositorRequestFailure),
    NiriConfigRead(Result<ConfiguredWorkspaces, String>),
    DesktopBus(Result<DesktopBusEvent, zbus::Error>),
    Interaction(ShellInteraction),
}

impl From<HoverCardMessage> for Message {
    fn from(message: HoverCardMessage) -> Self {
        Message::Interaction(ShellInteraction::from(message))
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

#[derive(Debug, Clone)]
pub enum ShellInteraction {
    Group(WidgetGroupInteraction),
    Popup(PopupMessage),
    Notification(NotificationMessage),
    DismissPressed,
    SidebarWidthChanged { window: Id, width: f32 },
    Workspaces(WorkspacesMessage),
    SurfaceRequest(SurfaceRequest),
    CompositorRequestFailed(CompositorRequestFailure),
}

impl From<ShellInteraction> for Message {
    fn from(interaction: ShellInteraction) -> Self {
        Message::Interaction(interaction)
    }
}

impl From<WidgetGroupInteraction> for ShellInteraction {
    fn from(group_interaction: WidgetGroupInteraction) -> Self {
        ShellInteraction::Group(group_interaction)
    }
}

impl From<WorkspacesMessage> for ShellInteraction {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        match workspaces_message {
            WorkspacesMessage::HoverCard(hover_card) => ShellInteraction::from(hover_card),
            other_message => ShellInteraction::Workspaces(other_message),
        }
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
        ShellInteraction::Popup(PopupMessage::HoverCard(message))
    }
}

impl From<GroupEvent<ShellInteraction>> for ShellInteraction {
    fn from(event: GroupEvent<ShellInteraction>) -> Self {
        match event {
            GroupEvent::Group(interaction) => interaction,
            GroupEvent::HoverCard(hover_card) => ShellInteraction::from(hover_card),
        }
    }
}

impl From<PopupMessage> for ShellInteraction {
    fn from(popup_message: PopupMessage) -> Self {
        ShellInteraction::Popup(popup_message)
    }
}
