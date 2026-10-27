use std::{collections::HashMap, sync::Arc};

use iced::window::Id;
use iced_exwlshell::{actions::RegionActionCB, to_layer_message};
use sottomano_config::{Config, LoadError};
use sottomano_sources::{
    SnapshotDelivery,
    desktop_notifications::alert_sound_player::SoundTheme,
    desktop_windows_apps::{
        app_launch_history::Launch, desktop_entry_app_icons::DesktopEntries,
        firefox_favicon_lookup::RawFavicon, niri_action_requests,
        niri_kdl_config::ConfiguredWorkspaces, niri_output_facts,
    },
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBusEvent,
};
use sottomano_widgets::{
    bar_slot_drag::DragMessage,
    desktop_index::ResolvedIcons,
    hover_tooltips::{PopupMessage, TooltipMessage},
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widget_groups::{
        audio_group::AudioMessage, caldav_group::CaldavMessage,
        claude_usage_group::ClaudeUsageMessage, clock_group::ClockMessage,
        color_temperature_group::ColorTemperatureMessage, compute_group::ComputeMessage,
        copilot_usage_group::CopilotUsageMessage, dark_mode_group::DarkModeMessage,
        disk_group::DiskMessage, gamepad_group::GamepadMessage, gpu_group::GpuMessage,
        idle_timeouts_group::IdleTimeoutsMessage, internet_group::InternetMessage,
        journal_events_group::JournalEventsMessage, llama_server_group::LlamaServerMessage,
        login_session_group::LoginSessionMessage, media_group::MediaMessage,
        memory_board_group::MemoryBoardMessage, monitor_group::MonitorMessage,
        net_interfaces_group::NetInterfacesMessage, niri_window_group::NiriWindowMessage,
        notification_group::NotificationGroupMessage,
        openrouter_credit_group::OpenRouterCreditMessage,
        overlay_settings_group::OverlaySettingsMessage, panel_settings_group::PanelSettingsMessage,
        pending_upgrades_group::PendingUpgradesMessage, power_group::PowerMessage,
        quiet_schedule_group::QuietScheduleMessage, session_focus_group::SessionFocusMessage,
        session_menu::SessionMessage, sidebar_settings_group::SidebarSettingsMessage,
        systemd_manager_group::SystemdManagerMessage, tray_group::TrayMessage,
        voice_dictation_group::VoiceDictationMessage, vpn_group::VpnMessage,
        wallpaper_group::WallpaperMessage, weather_group::WeatherMessage,
        window_group::WindowGroupMessage, workspaces_group::WorkspacesMessage,
    },
    widgets::{group_drawer::DrawerMessage, widget_group::GroupEvent},
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
    Disk(DiskMessage),
    PanelSettings(PanelSettingsMessage),
    SidebarSettings(SidebarSettingsMessage),
    Drawer(DrawerMessage),
    OverlaySettings(OverlaySettingsMessage),
    NiriWindow(NiriWindowMessage),
    Clock(ClockMessage),
    NotificationGroup(NotificationGroupMessage),
    QuietSchedule(QuietScheduleMessage),
    Session(SessionMessage),
    SystemdManager(SystemdManagerMessage),
    JournalEvents(JournalEventsMessage),
    PendingUpgrades(PendingUpgradesMessage),
    Internet(InternetMessage),
    Vpn(VpnMessage),
    NetInterfaces(NetInterfacesMessage),
    Monitor(MonitorMessage),
    ColorTemperature(ColorTemperatureMessage),
    DarkMode(DarkModeMessage),
    VoiceDictation(VoiceDictationMessage),
    IdleTimeouts(IdleTimeoutsMessage),
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
    SessionFocus(SessionFocusMessage),
    LoginSession(LoginSessionMessage),
    WindowGroup(WindowGroupMessage),
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
    CopyText(String),
    OpenUrl(String),
    RefreshHoveredReading,
    FocusWindow(u64),
    TooltipHideElapsed,
    Tooltip(TooltipMessage),
    Notification(NotificationMessage),
    OutputsResolved(HashMap<String, niri_output_facts::OutputInfo>),
    Output(OutputChange),
    ReloadDesktopEntries,
    SidebarWidthChanged { window: Id, width: f32 },
    PointerDrag(DragMessage),
    NiriActionFailed(niri_action_requests::ActionFailure),
    NiriConfigRead(Result<ConfiguredWorkspaces, String>),
    RestoreWorkspaceOrder { output: String },
    DesktopBus(Result<DesktopBusEvent, zbus::Error>),
}

impl From<TooltipMessage> for Message {
    fn from(message: TooltipMessage) -> Self {
        Message::Tooltip(message)
    }
}

impl From<GroupEvent<Message>> for Message {
    fn from(event: GroupEvent<Message>) -> Self {
        match event {
            GroupEvent::Group(message) => message,
            GroupEvent::Tooltip(tooltip) => Message::Tooltip(tooltip),
        }
    }
}

impl From<PopupMessage> for Message {
    fn from(popup_message: PopupMessage) -> Self {
        match popup_message {
            PopupMessage::Tooltip(tooltip) => Message::Tooltip(tooltip),
            PopupMessage::CopyText(copied) => Message::CopyText(copied),
            PopupMessage::OpenUrl(url) => Message::OpenUrl(url),
            PopupMessage::FocusWindow(window) => Message::FocusWindow(window),
            PopupMessage::RefreshHoveredReading => Message::RefreshHoveredReading,
            PopupMessage::Tray(tray) => Message::Tray(tray),
            PopupMessage::Session(session) => Message::Session(session),
            PopupMessage::WindowGroup(window_group) => Message::WindowGroup(window_group),
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
            WorkspacesMessage::Tooltip(tooltip) => Message::Tooltip(tooltip),
            WorkspacesMessage::Drag(drag) => Message::PointerDrag(drag),
            WorkspacesMessage::RestoreOrder { output } => Message::RestoreWorkspaceOrder { output },
        }
    }
}
