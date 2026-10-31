use sottomano_sources::desktop_windows_apps::niri_action_requests::ActionFailure;
use sottomano_widgets::{
    clock::Clock,
    hover_tooltips::{PopupMessage, TooltipMessage},
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widget_groups::{
        appearance_presets_group::AppearancePresetsMessage, audio_group::AudioMessage,
        caldav_group::CaldavMessage, claude_usage_group::ClaudeUsageMessage,
        compute_group::ComputeMessage, gamepad_group::GamepadMessage,
        idle_timeouts_group::IdleTimeoutsMessage, login_session_group::LoginSessionMessage,
        media_group::MediaMessage, memory_board_group::MemoryBoardMessage,
        voice_dictation_group::VoiceDictationMessage, vpn_group::VpnMessage,
        wallpaper_group::WallpaperMessage, weather_group::WeatherMessage,
        window_group::WindowGroupMessage, workspaces_group::WorkspacesMessage,
    },
    widgets::widget_group::GroupEvent,
};

#[derive(Debug, Clone)]
pub enum ShowcaseMessage {
    Frame(Clock),
    Compute(ComputeMessage),
    MemoryBoard(MemoryBoardMessage),
    Weather(WeatherMessage),
    ClaudeUsage(ClaudeUsageMessage),
    Media(MediaMessage),
    IdleTimeouts(IdleTimeoutsMessage),
    Wallpaper(WallpaperMessage),
    Audio(AudioMessage),
    Gamepad(GamepadMessage),
    VoiceDictation(VoiceDictationMessage),
    Caldav(CaldavMessage),
    Vpn(VpnMessage),
    LoginSession(LoginSessionMessage),
    WindowGroup(WindowGroupMessage),
    AppearancePresets(AppearancePresetsMessage),
    Tooltip(TooltipMessage),
    TooltipHideElapsed,
    Notification(NotificationMessage),
    OutsideShowcase,
}

impl From<GroupEvent<ShowcaseMessage>> for ShowcaseMessage {
    fn from(event: GroupEvent<ShowcaseMessage>) -> Self {
        match event {
            GroupEvent::Group(message) => message,
            GroupEvent::Tooltip(tooltip) => ShowcaseMessage::Tooltip(tooltip),
        }
    }
}

impl From<WorkspacesMessage> for ShowcaseMessage {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        match workspaces_message {
            WorkspacesMessage::Tooltip(tooltip) => ShowcaseMessage::Tooltip(tooltip),
            WorkspacesMessage::Drag(_) | WorkspacesMessage::RestoreOrder { .. } => {
                ShowcaseMessage::OutsideShowcase
            }
        }
    }
}

impl From<PopupMessage> for ShowcaseMessage {
    fn from(popup_message: PopupMessage) -> Self {
        match popup_message {
            PopupMessage::Tooltip(tooltip) => ShowcaseMessage::Tooltip(tooltip),
            PopupMessage::WindowGroup(window_group) => ShowcaseMessage::WindowGroup(window_group),
            PopupMessage::CopyText(_)
            | PopupMessage::OpenUrl(_)
            | PopupMessage::FocusWindow(_)
            | PopupMessage::RefreshHoveredReading
            | PopupMessage::ToggleGroupMenuPin { .. } => ShowcaseMessage::OutsideShowcase,
        }
    }
}

impl From<NotificationMessage> for ShowcaseMessage {
    fn from(notification: NotificationMessage) -> Self {
        ShowcaseMessage::Notification(notification)
    }
}

impl From<SurfaceRequest> for ShowcaseMessage {
    fn from(_surface_request: SurfaceRequest) -> Self {
        ShowcaseMessage::OutsideShowcase
    }
}

impl From<ActionFailure> for ShowcaseMessage {
    fn from(_failure: ActionFailure) -> Self {
        ShowcaseMessage::OutsideShowcase
    }
}
