use std::{collections::HashMap, sync::Arc};

use iced::window::Id;
use iced_exwlshell::{actions::RegionActionCB, to_layer_message};
use sottomano_compositor_model::{
    compositor_output_facts, compositor_requests::CompositorRequestFailure,
    compositor_window_state::CompositorUpdate,
};
use sottomano_desktop_sinks::desktop_notifications::alert_sound_player::SoundTheme;
use sottomano_desktop_sources::{
    delivery_cadence_drivers::ChangeWatchFailure,
    desktop_windows_apps::{
        app_launch_history::Launch, configured_workspace_order::ConfiguredWorkspaces,
        desktop_entries::DesktopEntries, firefox_favicon_lookup::RawFavicon,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBusEvent,
};
use sottomano_toml_config::{Config, LoadError};
use sottomano_widgets::{
    cascading_menus::{
        app_launcher_menu::AppLauncherMessage, window_menu_bar::WindowMenuBarMessage,
    },
    desktop_panels::panel_slot_drag::DragMessage,
    group_indicators::{
        indicator_group_content::GroupEvent, indicator_group_interaction::IndicatorGroupInteraction,
    },
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{HoverCardMessage, PopupMessage},
    icon_catalog::drawable_icon_catalog::CatalogMessage,
    layer_surfaces::SurfaceRequest,
    notification_toasts::toast_messages::NotificationMessage,
    output_windows::OutputWindowsMessage,
    panel_menus::{
        agenda_menu::{
            caldav_group::CaldavMessage, clock_group::ClockMessage,
            location_group::LocationMessage, notification_group::NotificationGroupMessage,
            quiet_schedule_group::QuietScheduleMessage, session_focus_group::SessionFocusMessage,
            weather_group::WeatherMessage,
        },
        appearance_menu::{
            compositor_styling_group::CompositorStylingMessage,
            overlay_settings_group::OverlaySettingsMessage,
            panel_settings_group::PanelSettingsMessage,
            sidebar_settings_group::SidebarSettingsMessage,
        },
        display_menu::{
            color_temperature_group::ColorTemperatureMessage, dark_mode_group::DarkModeMessage,
            monitor_group::MonitorMessage, wallpaper_group::WallpaperMessage,
        },
        gaming_menu::{gamepad_group::GamepadMessage, steam_client_groups::SteamClientMessage},
        hardware_menu::{
            compute_group::ComputeMessage, gpu_group::GpuMessage,
            memory_board_group::MemoryBoardMessage, physical_drive_group::DriveMessage,
            power_group::PowerMessage, storage_space_group::StorageMessage,
        },
        media_menu::{
            audio_group::AudioMessage, media_group::MediaMessage,
            voice_dictation_group::VoiceDictationMessage,
        },
        network_menu::{
            bluetooth_adapters_group::BluetoothMessage, ethernet_ports_group::EthernetPortsMessage,
            mobile_modems_group::MobileModemsMessage,
            network_manager_vpn_group::NetworkManagerVpnMessage,
            reachability_group::ReachabilityMessage, tailnet_group::TailnetMessage,
            wifi_adapters_group::WifiAdaptersMessage,
            wireguard_tunnels_group::WireguardTunnelsMessage,
        },
        services_menu::{
            ci_runs_group::CiRunsMessage, claude_usage_group::ClaudeUsageMessage,
            copilot_usage_group::CopilotUsageMessage, journal_events_group::JournalEventsMessage,
            llama_server_group::LlamaServerMessage,
            openrouter_credit_group::OpenRouterCreditMessage,
            pending_upgrades_group::PendingUpgradesMessage,
            systemd_manager_group::SystemdManagerMessage,
        },
        session_menu::{
            idle_pause_groups::IdlePauseMessage, login_session_group::LoginSessionMessage,
            tray_group::TrayMessage,
        },
        session_power_menu::session_power_group::SessionMessage,
        virtualization_menu::{
            apptainer_containers_group::ApptainerContainersMessage,
            cgroup_memory_limits_group::CgroupMemoryLimitsMessage,
            docker_engines_group::DockerEnginesMessage,
            flatpak_sandboxes_group::FlatpakSandboxesMessage,
            incus_instances_group::IncusInstancesMessage,
            lxc_containers_group::LxcContainersMessage, microvm_guests_group::MicrovmGuestsMessage,
            nspawn_containers_group::NspawnContainersMessage,
            podman_engines_group::PodmanEnginesMessage,
            virtual_machines_group::VirtualMachinesMessage,
            virtualbox_machines_group::VirtualBoxMachinesMessage,
            vmware_machines_group::VmwareMachinesMessage,
            waydroid_container_group::WaydroidContainerMessage,
        },
    },
    widgets::typing_under_cards::TypingSignal,
    workspace_switcher::WorkspacesMessage,
};
use strum::IntoStaticStr;

#[derive(Debug, Clone)]
pub enum OutputChange {
    Connected(String),
    Rearranged,
    Disconnected(String),
    Unavailable(String),
}

#[to_layer_message(multi)]
#[derive(Debug, IntoStaticStr)]
pub enum Message {
    Weather(WeatherMessage),
    Location(LocationMessage),
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
    SteamClient(SteamClientMessage),
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
    WindowMenuBar(WindowMenuBarMessage),
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
    OutputWindows(OutputWindowsMessage),
    AppLauncher(AppLauncherMessage),
    TrackerLoaded(Vec<Launch>),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    IconCatalog(CatalogMessage),
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
    IconThemeChanged(String),
    PointerDrag(DragMessage),
    CompositorRequestFailed(CompositorRequestFailure),
    ConfiguredWorkspacesRead(Result<ConfiguredWorkspaces, String>),
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

#[derive(Debug, Clone, IntoStaticStr)]
pub enum ShellInteraction {
    Group(IndicatorGroupInteraction),
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

impl From<IndicatorGroupInteraction> for Message {
    fn from(group_interaction: IndicatorGroupInteraction) -> Self {
        Message::Interaction(ShellInteraction::Group(group_interaction))
    }
}

impl From<IndicatorGroupInteraction> for ShellInteraction {
    fn from(group_interaction: IndicatorGroupInteraction) -> Self {
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

impl From<GroupEvent> for ShellInteraction {
    fn from(event: GroupEvent) -> Self {
        match event {
            GroupEvent::Interaction(interaction) => ShellInteraction::Group(interaction),
            GroupEvent::Popup(popup) => ShellInteraction::Popup(popup),
        }
    }
}

impl From<PopupMessage> for ShellInteraction {
    fn from(popup_message: PopupMessage) -> Self {
        ShellInteraction::Popup(popup_message)
    }
}
