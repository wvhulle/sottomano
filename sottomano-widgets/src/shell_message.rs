use std::{collections::HashMap, sync::Arc};

use iced::window::Id;
use iced_exwlshell::to_layer_message;
use sottomano_config::{Config, LoadError};
use sottomano_sources::{
    SourceDelivery,
    desktop_notifications::alert_sound_player::SoundTheme,
    desktop_windows_apps::{
        app_launch_history::Launch, desktop_entry_app_icons::DesktopEntries,
        firefox_favicon_lookup::RawFavicon, niri_action_requests,
        niri_kdl_config::ConfiguredWorkspaces, niri_output_facts,
        niri_window_placement::WindowOwner,
    },
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBusEvent,
};
use sottomano_state::{ShellSettings, niri_rules::StoredRules};

use crate::{
    bar_slot_drag::DragMessage,
    desktop_index::ResolvedIcons,
    hover_tooltips::TooltipMessage,
    toast_messages::NotificationMessage,
    widget_groups::{
        audio_group::AudioMessage, claude_usage_group::ClaudeUsageMessage,
        clock_group::ClockMessage, color_temperature_group::ColorTemperatureMessage,
        compute_group::ComputeMessage, copilot_usage_group::CopilotUsageMessage,
        dark_mode_group::DarkModeMessage, disk_group::DiskMessage, gamepad_group::GamepadMessage,
        gpu_group::GpuMessage, idle_timeouts_group::IdleTimeoutsMessage,
        internet_group::InternetMessage, journal_events_group::JournalEventsMessage,
        llama_server_group::LlamaServerMessage, login_session_group::LoginSessionMessage,
        media_group::MediaMessage, memory_board_group::MemoryBoardMessage,
        monitor_group::MonitorMessage, net_interfaces_group::NetInterfacesMessage,
        niri_window_group::NiriWindowMessage, notification_group::NotificationGroupMessage,
        openrouter_credit_group::OpenRouterCreditMessage,
        overlay_settings_group::OverlaySettingsMessage, panel_settings_group::PanelSettingsMessage,
        pending_upgrades_group::PendingUpgradesMessage, power_group::PowerMessage,
        quiet_schedule_group::QuietScheduleMessage, session_focus_group::SessionFocusMessage,
        session_menu::SessionMessage, sidebar_settings_group::SidebarSettingsMessage,
        systemd_manager_group::SystemdManagerMessage, tray_group::TrayMessage,
        voice_dictation_group::VoiceDictationMessage, wallpaper_group::WallpaperMessage,
        weather_group::WeatherMessage, window_group::WindowGroupMessage,
    },
    widgets::widget_group::GroupEvent,
};

#[derive(Debug, Clone)]
pub struct AppWindowFocus<M> {
    pub owner: WindowOwner,
    pub unmatched: M,
}

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
    Power(PowerMessage),
    Compute(ComputeMessage),
    MemoryBoard(MemoryBoardMessage),
    Gamepad(GamepadMessage),
    HostHardware(Box<SourceDelivery<HostHardware>>),
    Gpu(GpuMessage),
    Disk(DiskMessage),
    PanelSettings(PanelSettingsMessage),
    SidebarSettings(SidebarSettingsMessage),
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
    NetInterfaces(NetInterfacesMessage),
    Monitor(MonitorMessage),
    ColorTemperature(ColorTemperatureMessage),
    DarkMode(DarkModeMessage),
    VoiceDictation(VoiceDictationMessage),
    IdleTimeouts(IdleTimeoutsMessage),
    Media(MediaMessage),
    Tray(TrayMessage),
    ShellSettingsLoaded(ShellSettings),
    NiriRulesLoaded(StoredRules),
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
