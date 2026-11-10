use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::CompositorRequestFailure;
use sottomano_widgets::{
    clock::Clock,
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{HoverCardMessage, PopupMessage},
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widget_groups::{
        appearance_presets_group::AppearancePresetsMessage, audio_group::AudioMessage,
        caldav_group::CaldavMessage, cgroup_memory_limits_group::CgroupMemoryLimitsMessage,
        ci_runs_group::CiRunsMessage, claude_usage_group::ClaudeUsageMessage,
        compute_group::ComputeMessage, gamepad_group::GamepadMessage,
        idle_pause_groups::IdlePauseMessage, login_session_group::LoginSessionMessage,
        media_group::MediaMessage, memory_board_group::MemoryBoardMessage,
        nspawn_containers_group::NspawnContainersMessage,
        podman_engines_group::PodmanEnginesMessage, tailnet_group::TailnetMessage,
        virtual_machines_group::VirtualMachinesMessage,
        voice_dictation_group::VoiceDictationMessage, wallpaper_group::WallpaperMessage,
        weather_group::WeatherMessage, widget_group_content::GroupEvent,
        window_group::WindowGroupMessage, wireguard_tunnels_group::WireguardTunnelsMessage,
        workspaces_group::WorkspacesMessage,
    },
};

#[derive(Debug, Clone)]
pub enum ShowcaseMessage {
    Frame(Clock),
    Compute(ComputeMessage),
    MemoryBoard(MemoryBoardMessage),
    Weather(WeatherMessage),
    ClaudeUsage(ClaudeUsageMessage),
    Media(MediaMessage),
    IdleTimeouts(IdlePauseMessage),
    Wallpaper(WallpaperMessage),
    Audio(AudioMessage),
    Gamepad(GamepadMessage),
    VoiceDictation(VoiceDictationMessage),
    Caldav(CaldavMessage),
    PodmanEngines(PodmanEnginesMessage),
    VirtualMachines(VirtualMachinesMessage),
    NspawnContainers(NspawnContainersMessage),
    CgroupMemoryLimits(CgroupMemoryLimitsMessage),
    CiRuns(CiRunsMessage),
    Tailnet(TailnetMessage),
    WireguardTunnels(WireguardTunnelsMessage),
    LoginSession(LoginSessionMessage),
    WindowGroup(WindowGroupMessage),
    AppearancePresets(AppearancePresetsMessage),
    GroupMenuState(GroupMenuMessage),
    HoverCard(HoverCardMessage),
    HoverCardHideElapsed,
    Notification(NotificationMessage),
    OutsideShowcase,
}

impl From<GroupEvent<ShowcaseMessage>> for ShowcaseMessage {
    fn from(event: GroupEvent<ShowcaseMessage>) -> Self {
        match event {
            GroupEvent::Group(message) => message,
            GroupEvent::HoverCard(hover_card) => ShowcaseMessage::HoverCard(hover_card),
        }
    }
}

impl From<WorkspacesMessage> for ShowcaseMessage {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        match workspaces_message {
            WorkspacesMessage::HoverCard(hover_card) => ShowcaseMessage::HoverCard(hover_card),
            WorkspacesMessage::Drag(_)
            | WorkspacesMessage::RestoreOrder { .. }
            | WorkspacesMessage::ToggleEmptyWorkspaces => ShowcaseMessage::OutsideShowcase,
        }
    }
}

impl From<PopupMessage> for ShowcaseMessage {
    fn from(popup_message: PopupMessage) -> Self {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => ShowcaseMessage::HoverCard(hover_card),
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

impl From<CompositorRequestFailure> for ShowcaseMessage {
    fn from(_failure: CompositorRequestFailure) -> Self {
        ShowcaseMessage::OutsideShowcase
    }
}
