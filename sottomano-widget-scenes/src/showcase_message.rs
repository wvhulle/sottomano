use iced_exwlshell::SurfaceRegionsCollector;
use sottomano_compositor_model::compositor_requests::CompositorRequestFailure;
use sottomano_widgets::{
    clock::Clock,
    group_indicators::{
        indicator_group_content::GroupEvent, indicator_group_interaction::IndicatorGroupInteraction,
    },
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{HoverCardMessage, PopupMessage},
    layer_surfaces::SurfaceRequest,
    notification_toasts::toast_messages::NotificationMessage,
    output_windows::OutputWindowsMessage,
    panel_menus::{
        agenda_menu::{
            caldav_group::CaldavMessage, location_group::LocationMessage,
            weather_group::WeatherMessage,
        },
        display_menu::wallpaper_group::WallpaperMessage,
        gaming_menu::gamepad_group::GamepadMessage,
        hardware_menu::{compute_group::ComputeMessage, memory_board_group::MemoryBoardMessage},
        media_menu::{
            audio_group::AudioMessage, media_group::MediaMessage,
            voice_dictation_group::VoiceDictationMessage,
        },
        network_menu::{
            tailnet_group::TailnetMessage, wireguard_tunnels_group::WireguardTunnelsMessage,
        },
        services_menu::{ci_runs_group::CiRunsMessage, claude_usage_group::ClaudeUsageMessage},
        session_menu::{
            idle_pause_groups::IdlePauseMessage, login_session_group::LoginSessionMessage,
        },
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
    workspace_switcher::WorkspacesMessage,
};

#[derive(Debug)]
pub enum ShowcaseMessage {
    Frame(Clock),
    Compute(ComputeMessage),
    MemoryBoard(MemoryBoardMessage),
    Weather(WeatherMessage),
    Location(LocationMessage),
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
    OutputWindows(OutputWindowsMessage),
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
