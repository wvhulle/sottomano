use iced_exwlshell::SurfaceRegionsCollector;
use sottomano_compositor_model::compositor_requests::CompositorRequestFailure;
use sottomano_widgets::{
    clock::Clock,
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{HoverCardMessage, PopupMessage},
    indicator_groups::{
        apptainer_containers_group::ApptainerContainersMessage, audio_group::AudioMessage,
        caldav_group::CaldavMessage, cgroup_memory_limits_group::CgroupMemoryLimitsMessage,
        ci_runs_group::CiRunsMessage, claude_usage_group::ClaudeUsageMessage,
        compute_group::ComputeMessage, docker_engines_group::DockerEnginesMessage,
        flatpak_sandboxes_group::FlatpakSandboxesMessage, gamepad_group::GamepadMessage,
        idle_pause_groups::IdlePauseMessage, incus_instances_group::IncusInstancesMessage,
        indicator_group_content::GroupEvent,
        indicator_group_interaction::IndicatorGroupInteraction,
        login_session_group::LoginSessionMessage, lxc_containers_group::LxcContainersMessage,
        media_group::MediaMessage, memory_board_group::MemoryBoardMessage,
        microvm_guests_group::MicrovmGuestsMessage,
        nspawn_containers_group::NspawnContainersMessage,
        podman_engines_group::PodmanEnginesMessage, tailnet_group::TailnetMessage,
        virtual_machines_group::VirtualMachinesMessage,
        virtualbox_machines_group::VirtualBoxMachinesMessage,
        vmware_machines_group::VmwareMachinesMessage, voice_dictation_group::VoiceDictationMessage,
        wallpaper_group::WallpaperMessage, waydroid_container_group::WaydroidContainerMessage,
        weather_group::WeatherMessage, window_group::WindowGroupMessage,
        wireguard_tunnels_group::WireguardTunnelsMessage, workspaces_group::WorkspacesMessage,
    },
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
};

#[derive(Debug)]
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
    Tailnet(TailnetMessage),
    WireguardTunnels(WireguardTunnelsMessage),
    LoginSession(LoginSessionMessage),
    WindowGroup(WindowGroupMessage),
    GroupMenuState(GroupMenuMessage),
    HoverCardHideElapsed,
    Notification(NotificationMessage),
    SurfaceRegionsCollected(SurfaceRegionsCollector),
    Interaction(ShowcaseInteraction),
}

#[derive(Debug, Clone)]
pub enum ShowcaseInteraction {
    Group(IndicatorGroupInteraction),
    Popup(PopupMessage),
    Notification(NotificationMessage),
    OutsideShowcase,
}

impl From<ShowcaseInteraction> for ShowcaseMessage {
    fn from(interaction: ShowcaseInteraction) -> Self {
        ShowcaseMessage::Interaction(interaction)
    }
}

impl From<HoverCardMessage> for ShowcaseInteraction {
    fn from(hover_card: HoverCardMessage) -> Self {
        ShowcaseInteraction::Popup(PopupMessage::HoverCard(hover_card))
    }
}

impl From<IndicatorGroupInteraction> for ShowcaseInteraction {
    fn from(group_interaction: IndicatorGroupInteraction) -> Self {
        ShowcaseInteraction::Group(group_interaction)
    }
}

impl From<GroupEvent> for ShowcaseInteraction {
    fn from(event: GroupEvent) -> Self {
        match event {
            GroupEvent::Interaction(interaction) => ShowcaseInteraction::Group(interaction),
            GroupEvent::Popup(popup) => ShowcaseInteraction::Popup(popup),
        }
    }
}

impl From<WorkspacesMessage> for ShowcaseInteraction {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        match workspaces_message {
            WorkspacesMessage::HoverCard(hover_card) => ShowcaseInteraction::from(hover_card),
            WorkspacesMessage::Drag(_)
            | WorkspacesMessage::RestoreOrder { .. }
            | WorkspacesMessage::ToggleEmptyWorkspaces
            | WorkspacesMessage::Focus(_) => ShowcaseInteraction::OutsideShowcase,
        }
    }
}

impl From<PopupMessage> for ShowcaseInteraction {
    fn from(popup_message: PopupMessage) -> Self {
        ShowcaseInteraction::Popup(popup_message)
    }
}

impl From<NotificationMessage> for ShowcaseInteraction {
    fn from(notification: NotificationMessage) -> Self {
        ShowcaseInteraction::Notification(notification)
    }
}

impl From<SurfaceRequest> for ShowcaseInteraction {
    fn from(_surface_request: SurfaceRequest) -> Self {
        ShowcaseInteraction::OutsideShowcase
    }
}

impl From<CompositorRequestFailure> for ShowcaseInteraction {
    fn from(_failure: CompositorRequestFailure) -> Self {
        ShowcaseInteraction::OutsideShowcase
    }
}
