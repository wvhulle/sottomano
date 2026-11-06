use std::{collections::HashMap, sync::Arc};

use iced::window::Id;
use iced_exwlshell::{actions::RegionActionCB, to_layer_message};
use sottomano_desktop_sinks::{
    desktop_notifications::alert_sound_player::SoundTheme,
    desktop_windows_apps::niri_action_requests,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::{
        app_launch_history::Launch, desktop_entry_app_icons::DesktopEntries,
        firefox_favicon_lookup::RawFavicon, niri_kdl_config::ConfiguredWorkspaces,
        niri_output_facts,
    },
    host_machine_identity::udev_hardware_identity::HostHardware,
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
        app_launcher_group::AppLauncherMessage, appearance_presets_group::AppearancePresetsMessage,
        audio_group::AudioMessage, bluetooth_adapters_group::BluetoothMessage,
        caldav_group::CaldavMessage, claude_usage_group::ClaudeUsageMessage,
        clock_group::ClockMessage, color_temperature_group::ColorTemperatureMessage,
        compute_group::ComputeMessage, copilot_usage_group::CopilotUsageMessage,
        dark_mode_group::DarkModeMessage, ethernet_ports_group::EthernetPortsMessage,
        forgejo_tasks_group::ForgejoTasksMessage, gamepad_group::GamepadMessage,
        gpu_group::GpuMessage, idle_pause_groups::IdlePauseMessage,
        journal_events_group::JournalEventsMessage, llama_server_group::LlamaServerMessage,
        login_session_group::LoginSessionMessage, media_group::MediaMessage,
        memory_board_group::MemoryBoardMessage, mobile_modems_group::MobileModemsMessage,
        monitor_group::MonitorMessage, niri_window_group::NiriWindowMessage,
        notification_group::NotificationGroupMessage,
        openrouter_credit_group::OpenRouterCreditMessage,
        overlay_settings_group::OverlaySettingsMessage, panel_settings_group::PanelSettingsMessage,
        pending_upgrades_group::PendingUpgradesMessage, physical_drive_group::DriveMessage,
        podman_engines_group::PodmanEnginesMessage, power_group::PowerMessage,
        quiet_schedule_group::QuietScheduleMessage, reachability_group::ReachabilityMessage,
        session_focus_group::SessionFocusMessage, session_power_group::SessionMessage,
        sidebar_settings_group::SidebarSettingsMessage, storage_space_group::StorageMessage,
        systemd_manager_group::SystemdManagerMessage, tailnet_group::TailnetMessage,
        tray_group::TrayMessage, virtual_machines_group::VirtualMachinesMessage,
        voice_dictation_group::VoiceDictationMessage, vpn_group::VpnMessage,
        wallpaper_group::WallpaperMessage, weather_group::WeatherMessage,
        widget_group_content::GroupEvent, wifi_adapters_group::WifiAdaptersMessage,
        window_group::WindowGroupMessage, wireguard_server_group::WireguardServerMessage,
        woodpecker_tasks_group::WoodpeckerTasksMessage, workspaces_group::WorkspacesMessage,
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
#[derive(Debug, Clone)]
pub enum Message {
    Weather(WeatherMessage),
    Wallpaper(WallpaperMessage),
    Caldav(CaldavMessage),
    Power(PowerMessage),
    Compute(ComputeMessage),
    MemoryBoard(MemoryBoardMessage),
    Gamepad(GamepadMessage),
    HostHardware(Box<SnapshotDelivery<HostHardware>>),
    Gpu(GpuMessage),
    Storage(StorageMessage),
    Drive(DriveMessage),
    PanelSettings(PanelSettingsMessage),
    SidebarSettings(SidebarSettingsMessage),
    AppearancePresets(AppearancePresetsMessage),
    OverlaySettings(OverlaySettingsMessage),
    NiriWindow(NiriWindowMessage),
    Clock(ClockMessage),
    NotificationGroup(NotificationGroupMessage),
    QuietSchedule(QuietScheduleMessage),
    Session(SessionMessage),
    SystemdManager(SystemdManagerMessage),
    JournalEvents(JournalEventsMessage),
    PendingUpgrades(PendingUpgradesMessage),
    Reachability(ReachabilityMessage),
    Tailnet(TailnetMessage),
    WireguardServer(WireguardServerMessage),
    Vpn(VpnMessage),
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
    ConfigFileChanged,
    ConfigLoaded(Result<Box<Config>, Arc<LoadError>>),
    SoundThemeLoaded(SoundTheme),
    Audio(AudioMessage),
    ClaudeUsage(ClaudeUsageMessage),
    CopilotUsage(CopilotUsageMessage),
    OpenRouterCredit(OpenRouterCreditMessage),
    LlamaServer(LlamaServerMessage),
    PodmanEngines(PodmanEnginesMessage),
    VirtualMachines(VirtualMachinesMessage),
    WoodpeckerTasks(WoodpeckerTasksMessage),
    ForgejoTasks(ForgejoTasksMessage),
    SessionFocus(SessionFocusMessage),
    LoginSession(LoginSessionMessage),
    WindowGroup(WindowGroupMessage),
    AppLauncher(AppLauncherMessage),
    TrackerLoaded(Vec<Launch>),
    DesktopEntriesLoaded(Arc<DesktopEntries>),
    IconsResolved(ResolvedIcons),
    FaviconsResolved(HashMap<String, RawFavicon>),
    FirefoxProfileChanged,
    DismissPressed,
    Niri(Vec<niri_ipc::Event>),
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
    OutputsResolved(HashMap<String, niri_output_facts::OutputInfo>),
    Output(OutputChange),
    ReloadDesktopEntries,
    SidebarWidthChanged {
        window: Id,
        width: f32,
    },
    PointerDrag(DragMessage),
    NiriActionFailed(niri_action_requests::ActionFailure),
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

impl From<GroupEvent<Message>> for Message {
    fn from(event: GroupEvent<Message>) -> Self {
        match event {
            GroupEvent::Group(message) => message,
            GroupEvent::HoverCard(hover_card) => Message::HoverCard(hover_card),
        }
    }
}

impl From<PopupMessage> for Message {
    fn from(popup_message: PopupMessage) -> Self {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => Message::HoverCard(hover_card),
            PopupMessage::CopyText(copied) => Message::CopyText(copied),
            PopupMessage::OpenUrl(url) => Message::OpenUrl(url),
            PopupMessage::FocusWindow(window) => Message::FocusWindow(window),
            PopupMessage::RefreshHoveredReading => Message::RefreshHoveredReading,
            PopupMessage::WindowGroup(window_group) => Message::WindowGroup(window_group),
            PopupMessage::ToggleGroupMenuPin { menu, entry } => {
                Message::UnpinFromPanel { menu, entry }
            }
        }
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

impl From<niri_action_requests::ActionFailure> for Message {
    fn from(failure: niri_action_requests::ActionFailure) -> Self {
        Message::NiriActionFailed(failure)
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
