use iced::Element;
use sottomano_compositor_model::compositor_window_state::CompositorWindowState;
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::NotificationData,
    desktop_windows_apps::desktop_entries::DesktopEntries,
};
use sottomano_toml_config::{
    Config, panel_menus::gaming_menu::gamepad_bindings::GamepadBindingsConfig,
    screen_edges::Position,
};
use sottomano_widgets::{
    cascading_menus::app_launcher_menu::AppLauncherMenu,
    clock::Clock,
    group_indicators::{
        group_indicator::HoverFetch, indicator_group_content::IndicatorGroupContent,
    },
    hover_cards::{
        CardHeader, HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard,
    },
    icon_catalog::drawable_icon_catalog::IconCatalog,
    output_windows::WindowInspection,
    panel_menus::{
        agenda_menu::{
            caldav_group::CaldavGroup, location_group::LocationGroup,
            session_focus_group::SessionFocusGroup, weather_group::WeatherGroup,
        },
        display_menu::wallpaper_group::WallpaperGroup,
        gaming_menu::gamepad_group::GamepadGroup,
        hardware_menu::{compute_group::ComputeGroup, memory_board_group::MemoryBoardGroup},
        media_menu::{
            audio_group::AudioGroup, media_group::MediaGroup,
            voice_dictation_group::VoiceDictationGroup,
        },
        network_menu::{
            tailnet_group::TailnetGroup, wireguard_tunnels_group::WireguardTunnelsGroup,
        },
        services_menu::{ci_runs_group::CiRunsGroup, claude_usage_group::ClaudeUsageGroup},
        session_menu::{
            idle_pause_groups::IdlePauseGroups, login_session_group::LoginSessionGroup,
        },
        settings_menu::appearance_presets_group::AppearancePresetsGroup,
        virtualization_menu::{
            apptainer_containers_group::ApptainerContainersGroup,
            cgroup_memory_limits_group::CgroupMemoryLimitsGroup,
            docker_engines_group::DockerEnginesGroup,
            flatpak_sandboxes_group::FlatpakSandboxesGroup,
            incus_instances_group::IncusInstancesGroup, lxc_containers_group::LxcContainersGroup,
            microvm_guests_group::MicrovmGuestsGroup,
            nspawn_containers_group::NspawnContainersGroup,
            podman_engines_group::PodmanEnginesGroup, virtual_machines_group::VirtualMachinesGroup,
            virtualbox_machines_group::VirtualBoxMachinesGroup,
            vmware_machines_group::VmwareMachinesGroup,
            waydroid_container_group::WaydroidContainerGroup,
        },
    },
    widgets::appearance_presets::{AppearancePreset, ShellAppearance},
};

use crate::{
    fixtures::{desktop_session::terminal_hover_inspection, hover_fetch_replies},
    showcase_message::{ShowcaseInteraction, ShowcaseMessage},
    showcased_group::{
        showcased_desktop_session::ShowcasedDesktopSession,
        showcased_shell_settings::ShowcasedShellSettings, showcased_toasts::ShowcasedToasts,
    },
};

mod group_indicator_contents;
mod group_interactions;
mod group_message_updates;
pub mod showcased_desktop_session;
pub mod showcased_shell_settings;
pub mod showcased_toasts;

pub const SHOWCASE_OUTPUT: &str = "widget-showcase";

#[must_use]
pub fn recorded_look() -> ShellAppearance {
    AppearancePreset::ClearDark.applied_to(ShellAppearance::default())
}

pub enum ShowcasedGroup {
    Compute(Box<ComputeGroup>),
    MemoryBoard(Box<MemoryBoardGroup>),
    Weather(Box<WeatherGroup>),
    Location(Box<LocationGroup>),
    ClaudeUsage(Box<ClaudeUsageGroup>),
    Media(Box<ShowcasedMedia>),
    IdleTimeouts(Box<IdlePauseGroups>),
    AppLauncher(Box<ShowcasedAppLauncher>),
    AppearancePresets(Box<ShowcasedAppearancePresets>),
    Wallpaper(Box<WallpaperGroup>),
    Audio(Box<AudioGroup>),
    Gamepad(Box<ShowcasedGamepad>),
    VoiceDictation(Box<VoiceDictationGroup>),
    Caldav(Box<CaldavGroup>),
    PodmanEngines(Box<PodmanEnginesGroup>),
    VirtualMachines(Box<VirtualMachinesGroup>),
    NspawnContainers(Box<NspawnContainersGroup>),
    CgroupMemoryLimits(Box<CgroupMemoryLimitsGroup>),
    DockerEngines(Box<DockerEnginesGroup>),
    ApptainerContainers(Box<ApptainerContainersGroup>),
    VmwareMachines(Box<VmwareMachinesGroup>),
    VirtualBoxMachines(Box<VirtualBoxMachinesGroup>),
    WaydroidContainer(Box<WaydroidContainerGroup>),
    FlatpakSandboxes(Box<FlatpakSandboxesGroup>),
    LxcContainers(Box<LxcContainersGroup>),
    IncusInstances(Box<IncusInstancesGroup>),
    MicrovmGuests(Box<MicrovmGuestsGroup>),
    CiRuns(Box<CiRunsGroup>),
    Tailnet(Box<TailnetGroup>),
    WireguardTunnels(Box<WireguardTunnelsGroup>),
    LoginSession(Box<LoginSessionGroup>),
    SessionFocus(Box<SessionFocusGroup>),
    BottomPanel(Box<ShowcasedDesktopSession>),
    WorkspaceSidebar(Box<ShowcasedDesktopSession>),
    Toasts(Box<ShowcasedToasts>),
    MenuMembers(Vec<ShowcasedGroup>),
    ShellSettings(Box<ShowcasedShellSettings>),
}

pub struct ShowcasedAppearancePresets {
    pub group: AppearancePresetsGroup,
    pub appearance: ShellAppearance,
}

pub struct ShowcasedAppLauncher {
    pub group: AppLauncherMenu,
    pub icon_catalog: IconCatalog,
}

pub struct ShowcasedGamepad {
    pub group: GamepadGroup,
    pub bindings: GamepadBindingsConfig,
}

pub struct ShowcasedMedia {
    pub group: MediaGroup,
    pub desktop_entries: DesktopEntries,
}

impl ShowcasedGroup {
    pub fn position(&self) -> Position {
        match self {
            ShowcasedGroup::BottomPanel(_)
            | ShowcasedGroup::AppLauncher(_)
            | ShowcasedGroup::ClaudeUsage(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::AppearancePresets(_)
            | ShowcasedGroup::LoginSession(_)
            | ShowcasedGroup::ShellSettings(_) => Position::Bottom,
            ShowcasedGroup::WorkspaceSidebar(_) => Position::Left,
            ShowcasedGroup::Compute(_)
            | ShowcasedGroup::MemoryBoard(_)
            | ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Location(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::VoiceDictation(_)
            | ShowcasedGroup::Caldav(_)
            | ShowcasedGroup::SessionFocus(_)
            | ShowcasedGroup::PodmanEngines(_)
            | ShowcasedGroup::VirtualMachines(_)
            | ShowcasedGroup::NspawnContainers(_)
            | ShowcasedGroup::CgroupMemoryLimits(_)
            | ShowcasedGroup::DockerEngines(_)
            | ShowcasedGroup::ApptainerContainers(_)
            | ShowcasedGroup::VmwareMachines(_)
            | ShowcasedGroup::VirtualBoxMachines(_)
            | ShowcasedGroup::WaydroidContainer(_)
            | ShowcasedGroup::FlatpakSandboxes(_)
            | ShowcasedGroup::LxcContainers(_)
            | ShowcasedGroup::IncusInstances(_)
            | ShowcasedGroup::MicrovmGuests(_)
            | ShowcasedGroup::CiRuns(_)
            | ShowcasedGroup::Tailnet(_)
            | ShowcasedGroup::WireguardTunnels(_)
            | ShowcasedGroup::Toasts(_) => Position::Top,
            ShowcasedGroup::MenuMembers(groups) => groups
                .first()
                .map_or(Position::Top, ShowcasedGroup::position),
        }
    }

    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        match self {
            ShowcasedGroup::AppLauncher(_) => Vec::new(),
            ShowcasedGroup::MenuMembers(groups) => groups
                .iter()
                .flat_map(|member| member.group_contents(config, clock))
                .collect(),
            ShowcasedGroup::ShellSettings(settings) => settings.group_contents(config),
            interactive_group => interactive_group
                .interactive_group_contents(config, clock)
                .into_iter()
                .collect(),
        }
    }

    pub fn panel_groups<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<Element<'view, ShowcaseInteraction>> {
        let position = self.position();
        match self {
            ShowcasedGroup::BottomPanel(session) => session.workspaces_and_windows(config, clock),
            ShowcasedGroup::WorkspaceSidebar(session) => session.sidebar_groups(config, clock),
            _ => self
                .group_contents(config, clock)
                .into_iter()
                .map(|group| group.view_events(config, position, clock))
                .map(|element| element.map(ShowcaseInteraction::from))
                .collect(),
        }
    }

    pub fn start_hovered_fetch(
        &mut self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Vec<ShowcaseMessage> {
        if let ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) =
            self
        {
            let _watched_window = session.output_windows.follow_window_inspection(
                &session.compositor,
                WindowInspection::from(Some(target)),
                config,
                clock,
            );
            return terminal_hover_inspection::on_hover(target)
                .into_iter()
                .map(ShowcaseMessage::OutputWindows)
                .collect();
        }
        let hovered_fetch = self
            .group_contents(config, clock)
            .iter()
            .find_map(|group| group.hovered_indicator(target))
            .map(|hovered_row| hovered_row.hover_fetch.clone());
        let Some(HoverFetch::Fetch {
            start: started_fetch,
            ..
        }) = hovered_fetch
        else {
            return Vec::new();
        };
        let fetch_replies = hover_fetch_replies(&started_fetch);
        let _started = self.interact(*started_fetch, config, clock);
        fetch_replies
    }

    pub fn described_hover_card(
        &self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Option<HoverCard> {
        let indicator_card = self
            .group_contents(config, clock)
            .iter()
            .find_map(|group| group.hover_card(target, clock));
        if let Some(indicator_card) = indicator_card {
            return HoverCard::new(indicator_card.sections, indicator_card.header);
        }
        let standalone_sections = match self {
            ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                session.standalone_hover_card(target, config, clock)
            }
            _ => Vec::new(),
        };
        HoverCard::new(standalone_sections, CardHeader::Unheaded)
    }

    pub fn compositor(&self) -> Option<&CompositorWindowState> {
        match self {
            ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                Some(&session.compositor)
            }
            ShowcasedGroup::Toasts(toasts) => Some(&toasts.compositor),
            ShowcasedGroup::AppLauncher(_)
            | ShowcasedGroup::Compute(_)
            | ShowcasedGroup::MemoryBoard(_)
            | ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Location(_)
            | ShowcasedGroup::ClaudeUsage(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::VoiceDictation(_)
            | ShowcasedGroup::Caldav(_)
            | ShowcasedGroup::SessionFocus(_)
            | ShowcasedGroup::PodmanEngines(_)
            | ShowcasedGroup::VirtualMachines(_)
            | ShowcasedGroup::NspawnContainers(_)
            | ShowcasedGroup::CgroupMemoryLimits(_)
            | ShowcasedGroup::DockerEngines(_)
            | ShowcasedGroup::ApptainerContainers(_)
            | ShowcasedGroup::VmwareMachines(_)
            | ShowcasedGroup::VirtualBoxMachines(_)
            | ShowcasedGroup::WaydroidContainer(_)
            | ShowcasedGroup::FlatpakSandboxes(_)
            | ShowcasedGroup::LxcContainers(_)
            | ShowcasedGroup::IncusInstances(_)
            | ShowcasedGroup::MicrovmGuests(_)
            | ShowcasedGroup::CiRuns(_)
            | ShowcasedGroup::Tailnet(_)
            | ShowcasedGroup::WireguardTunnels(_)
            | ShowcasedGroup::LoginSession(_)
            | ShowcasedGroup::MenuMembers(_)
            | ShowcasedGroup::ShellSettings(_)
            | ShowcasedGroup::AppearancePresets(_) => None,
        }
    }

    pub fn opening_toasts(&self) -> &[NotificationData] {
        match self {
            ShowcasedGroup::Toasts(toasts) => &toasts.opening,
            _ => &[],
        }
    }

    pub fn backdrop(&self) -> Option<Element<'_, ShowcaseInteraction>> {
        match self {
            ShowcasedGroup::Toasts(toasts) => Some(toasts.window_frames()),
            _ => None,
        }
    }
}
