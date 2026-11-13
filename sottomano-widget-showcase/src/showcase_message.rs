use iced_exwlshell::SurfaceRegionsCollector;
use sottomano_desktop_sinks::compositor_window_requests::compositor_requests::CompositorRequestFailure;
use sottomano_widgets::{
    clock::Clock,
    group_menus::group_menu_state::GroupMenuMessage,
    hover_cards::{HoverCardMessage, PopupMessage},
    layer_surfaces::SurfaceRequest,
    toast_messages::NotificationMessage,
    widget_groups::{
        appearance_presets_group::AppearancePresetsMessage,
        apptainer_containers_group::ApptainerContainersMessage,
        audio_group::AudioMessage,
        caldav_group::CaldavMessage,
        cgroup_memory_limits_group::{CgroupMemoryLimitsInteraction, CgroupMemoryLimitsMessage},
        ci_runs_group::{CiRunsInteraction, CiRunsMessage},
        claude_usage_group::ClaudeUsageMessage,
        compute_group::{ComputeInteraction, ComputeMessage},
        docker_engines_group::DockerEnginesMessage,
        flatpak_sandboxes_group::FlatpakSandboxesMessage,
        gamepad_group::{GamepadInteraction, GamepadMessage},
        idle_pause_groups::{IdlePauseInteraction, IdlePauseMessage},
        incus_instances_group::{IncusInstancesInteraction, IncusInstancesMessage},
        login_session_group::LoginSessionMessage,
        lxc_containers_group::LxcContainersMessage,
        media_group::{MediaInteraction, MediaMessage},
        memory_board_group::MemoryBoardMessage,
        microvm_guests_group::{MicrovmGuestsInteraction, MicrovmGuestsMessage},
        nspawn_containers_group::{NspawnContainersInteraction, NspawnContainersMessage},
        podman_engines_group::PodmanEnginesMessage,
        tailnet_group::{TailnetInteraction, TailnetMessage},
        virtual_machines_group::{VirtualMachinesInteraction, VirtualMachinesMessage},
        virtualbox_machines_group::{VirtualBoxMachinesInteraction, VirtualBoxMachinesMessage},
        vmware_machines_group::VmwareMachinesMessage,
        voice_dictation_group::{VoiceDictationInteraction, VoiceDictationMessage},
        wallpaper_group::WallpaperMessage,
        waydroid_container_group::{WaydroidContainerInteraction, WaydroidContainerMessage},
        weather_group::WeatherMessage,
        widget_group_content::{GroupEvent, WidgetGroupExpansionMessage},
        window_group::{WindowGroupInteraction, WindowGroupMessage},
        wireguard_tunnels_group::{WireguardTunnelsInteraction, WireguardTunnelsMessage},
        workspaces_group::WorkspacesMessage,
    },
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
    AppearancePresets(AppearancePresetsMessage),
    GroupMenuState(GroupMenuMessage),
    HoverCard(HoverCardMessage),
    HoverCardHideElapsed,
    Notification(NotificationMessage),
    SurfaceRegionsCollected(SurfaceRegionsCollector),
    OutsideShowcase,
}

#[derive(Debug, Clone)]
pub enum ShowcaseInteraction {
    Compute(ComputeInteraction),
    MemoryBoard(WidgetGroupExpansionMessage),
    Weather(WidgetGroupExpansionMessage),
    ClaudeUsage(WidgetGroupExpansionMessage),
    Media(MediaInteraction),
    IdleTimeouts(IdlePauseInteraction),
    Wallpaper(WidgetGroupExpansionMessage),
    Audio(AudioMessage),
    Gamepad(GamepadInteraction),
    VoiceDictation(VoiceDictationInteraction),
    Caldav(WidgetGroupExpansionMessage),
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
    Tailnet(TailnetInteraction),
    WireguardTunnels(WireguardTunnelsInteraction),
    LoginSession(WidgetGroupExpansionMessage),
    WindowGroup(WindowGroupInteraction),
    AppearancePresets(AppearancePresetsMessage),
    GroupMenuState(GroupMenuMessage),
    HoverCard(HoverCardMessage),
    Notification(NotificationMessage),
    OutsideShowcase,
}

impl From<ShowcaseInteraction> for ShowcaseMessage {
    fn from(interaction: ShowcaseInteraction) -> Self {
        match interaction {
            ShowcaseInteraction::Compute(interaction) => {
                ShowcaseMessage::Compute(ComputeMessage::Interaction(interaction))
            }
            ShowcaseInteraction::MemoryBoard(expansion) => {
                ShowcaseMessage::MemoryBoard(MemoryBoardMessage::Expansion(expansion))
            }
            ShowcaseInteraction::Weather(expansion) => {
                ShowcaseMessage::Weather(WeatherMessage::Expansion(expansion))
            }
            ShowcaseInteraction::ClaudeUsage(expansion) => {
                ShowcaseMessage::ClaudeUsage(ClaudeUsageMessage::Expansion(expansion))
            }
            ShowcaseInteraction::Media(interaction) => {
                ShowcaseMessage::Media(MediaMessage::Interaction(interaction))
            }
            ShowcaseInteraction::IdleTimeouts(interaction) => {
                ShowcaseMessage::IdleTimeouts(IdlePauseMessage::Interaction(interaction))
            }
            ShowcaseInteraction::Wallpaper(expansion) => {
                ShowcaseMessage::Wallpaper(WallpaperMessage::Expansion(expansion))
            }
            ShowcaseInteraction::Audio(message) => ShowcaseMessage::Audio(message),
            ShowcaseInteraction::Gamepad(interaction) => {
                ShowcaseMessage::Gamepad(GamepadMessage::Interaction(interaction))
            }
            ShowcaseInteraction::VoiceDictation(interaction) => {
                ShowcaseMessage::VoiceDictation(VoiceDictationMessage::Interaction(interaction))
            }
            ShowcaseInteraction::Caldav(expansion) => {
                ShowcaseMessage::Caldav(CaldavMessage::Expansion(expansion))
            }
            ShowcaseInteraction::PodmanEngines(expansion) => {
                ShowcaseMessage::PodmanEngines(PodmanEnginesMessage::Expansion(expansion))
            }
            ShowcaseInteraction::VirtualMachines(interaction) => {
                ShowcaseMessage::VirtualMachines(VirtualMachinesMessage::Interaction(interaction))
            }
            ShowcaseInteraction::NspawnContainers(interaction) => {
                ShowcaseMessage::NspawnContainers(NspawnContainersMessage::Interaction(interaction))
            }
            ShowcaseInteraction::CgroupMemoryLimits(interaction) => {
                ShowcaseMessage::CgroupMemoryLimits(CgroupMemoryLimitsMessage::Interaction(
                    interaction,
                ))
            }
            ShowcaseInteraction::DockerEngines(expansion) => {
                ShowcaseMessage::DockerEngines(DockerEnginesMessage::Expansion(expansion))
            }
            ShowcaseInteraction::ApptainerContainers(expansion) => {
                ShowcaseMessage::ApptainerContainers(ApptainerContainersMessage::Expansion(
                    expansion,
                ))
            }
            ShowcaseInteraction::VmwareMachines(expansion) => {
                ShowcaseMessage::VmwareMachines(VmwareMachinesMessage::Expansion(expansion))
            }
            ShowcaseInteraction::VirtualBoxMachines(interaction) => {
                ShowcaseMessage::VirtualBoxMachines(VirtualBoxMachinesMessage::Interaction(
                    interaction,
                ))
            }
            ShowcaseInteraction::WaydroidContainer(interaction) => {
                ShowcaseMessage::WaydroidContainer(WaydroidContainerMessage::Interaction(
                    interaction,
                ))
            }
            ShowcaseInteraction::FlatpakSandboxes(expansion) => {
                ShowcaseMessage::FlatpakSandboxes(FlatpakSandboxesMessage::Expansion(expansion))
            }
            ShowcaseInteraction::LxcContainers(expansion) => {
                ShowcaseMessage::LxcContainers(LxcContainersMessage::Expansion(expansion))
            }
            ShowcaseInteraction::IncusInstances(interaction) => {
                ShowcaseMessage::IncusInstances(IncusInstancesMessage::Interaction(interaction))
            }
            ShowcaseInteraction::MicrovmGuests(interaction) => {
                ShowcaseMessage::MicrovmGuests(MicrovmGuestsMessage::Interaction(interaction))
            }
            ShowcaseInteraction::CiRuns(interaction) => {
                ShowcaseMessage::CiRuns(CiRunsMessage::Interaction(interaction))
            }
            ShowcaseInteraction::Tailnet(interaction) => {
                ShowcaseMessage::Tailnet(TailnetMessage::Interaction(interaction))
            }
            ShowcaseInteraction::WireguardTunnels(interaction) => {
                ShowcaseMessage::WireguardTunnels(WireguardTunnelsMessage::Interaction(interaction))
            }
            ShowcaseInteraction::LoginSession(expansion) => {
                ShowcaseMessage::LoginSession(LoginSessionMessage::Expansion(expansion))
            }
            ShowcaseInteraction::WindowGroup(interaction) => {
                ShowcaseMessage::WindowGroup(WindowGroupMessage::Interaction(interaction))
            }
            ShowcaseInteraction::AppearancePresets(message) => {
                ShowcaseMessage::AppearancePresets(message)
            }
            ShowcaseInteraction::GroupMenuState(message) => {
                ShowcaseMessage::GroupMenuState(message)
            }
            ShowcaseInteraction::HoverCard(message) => ShowcaseMessage::HoverCard(message),
            ShowcaseInteraction::Notification(message) => ShowcaseMessage::Notification(message),
            ShowcaseInteraction::OutsideShowcase => ShowcaseMessage::OutsideShowcase,
        }
    }
}

impl From<GroupEvent<ShowcaseInteraction>> for ShowcaseInteraction {
    fn from(event: GroupEvent<ShowcaseInteraction>) -> Self {
        match event {
            GroupEvent::Group(message) => message,
            GroupEvent::HoverCard(hover_card) => ShowcaseInteraction::HoverCard(hover_card),
        }
    }
}

impl From<WorkspacesMessage> for ShowcaseInteraction {
    fn from(workspaces_message: WorkspacesMessage) -> Self {
        match workspaces_message {
            WorkspacesMessage::HoverCard(hover_card) => ShowcaseInteraction::HoverCard(hover_card),
            WorkspacesMessage::Drag(_)
            | WorkspacesMessage::RestoreOrder { .. }
            | WorkspacesMessage::ToggleEmptyWorkspaces => ShowcaseInteraction::OutsideShowcase,
        }
    }
}

impl From<PopupMessage> for ShowcaseInteraction {
    fn from(popup_message: PopupMessage) -> Self {
        match popup_message {
            PopupMessage::HoverCard(hover_card) => ShowcaseInteraction::HoverCard(hover_card),
            PopupMessage::WindowGroup(window_group) => {
                ShowcaseInteraction::WindowGroup(window_group)
            }
            PopupMessage::CopyText(_)
            | PopupMessage::OpenUrl(_)
            | PopupMessage::FocusWindow(_)
            | PopupMessage::RefreshHoveredReading
            | PopupMessage::ReleaseFromPanel { .. } => ShowcaseInteraction::OutsideShowcase,
        }
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
