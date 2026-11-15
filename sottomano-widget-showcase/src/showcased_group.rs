use std::{collections::HashMap, ops::ControlFlow};

use corredo::theming::{background_color, primary_color, secondary_text_color, success_color};
use iced::{
    Color, Element, Font, Length, Padding, Theme, border,
    widget::{Column, Row, container, stack, text},
};
use iced_controller_diagram::ControllerArtwork;
use sottomano_desktop_sources::{
    compositor_window_ipc::{
        compositor_styling_model::AppWindowLayout, compositor_window_state::CompositorWindowState,
        niri_kdl_config::ConfiguredWorkspaces,
    },
    desktop_notifications::notification_data::NotificationData,
    desktop_windows_apps::workspace_window_snapshot::Snapshot,
};
use sottomano_toml_config::{
    Config, screen_edges::Position, widget_groups::gamepad_bindings::GamepadBindingsConfig,
};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    desktop_panels::{panel_frame::outlined_sides_inward_of, panel_slot_drag::PanelSlotContext},
    group_items::group_item::HoverFetch,
    group_menus::group_menu_state::GroupMenuState,
    hover_cards::{
        CardHeader, HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        app_launcher_group::AppLauncherGroup,
        appearance_presets_group::{AppearancePresetsGroup, AppearancePresetsMessage},
        apptainer_containers_group::ApptainerContainersGroup,
        audio_group::AudioGroup,
        caldav_group::CaldavGroup,
        cgroup_memory_limits_group::CgroupMemoryLimitsGroup,
        ci_runs_group::CiRunsGroup,
        claude_usage_group::ClaudeUsageGroup,
        compositor_styling_group::CompositorStylingGroup,
        compute_group::ComputeGroup,
        docker_engines_group::DockerEnginesGroup,
        flatpak_sandboxes_group::FlatpakSandboxesGroup,
        gamepad_group::GamepadGroup,
        idle_pause_groups::IdlePauseGroups,
        incus_instances_group::IncusInstancesGroup,
        login_session_group::LoginSessionGroup,
        lxc_containers_group::LxcContainersGroup,
        media_group::MediaGroup,
        memory_board_group::MemoryBoardGroup,
        microvm_guests_group::MicrovmGuestsGroup,
        nspawn_containers_group::NspawnContainersGroup,
        overlay_settings_group::OverlaySettingsGroup,
        panel_settings_group::PanelSettingsGroup,
        podman_engines_group::PodmanEnginesGroup,
        sidebar_settings_group::SidebarSettingsGroup,
        tailnet_group::TailnetGroup,
        virtual_machines_group::VirtualMachinesGroup,
        virtualbox_machines_group::VirtualBoxMachinesGroup,
        vmware_machines_group::VmwareMachinesGroup,
        voice_dictation_group::VoiceDictationGroup,
        wallpaper_group::WallpaperGroup,
        waydroid_container_group::WaydroidContainerGroup,
        weather_group::WeatherGroup,
        widget_group_content::WidgetGroupContent,
        window_group::{
            WindowGroup, WindowInspection, WindowRowLayout, WindowTarget, WindowTitleLength,
        },
        wireguard_tunnels_group::WireguardTunnelsGroup,
        workspaces_group::{
            EmptyWorkspaceFolding, EmptyWorkspaces, inline_strip, sidebar_column,
            workspace_hover_card,
        },
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        favicon::Favicons,
        pointer_drag::DragView,
        window_icon::WindowAssets,
    },
};

use crate::{
    fixtures::{desktop_session, hover_fetch_replies},
    showcase_message::{ShowcaseInteraction, ShowcaseMessage},
    showcase_program::SHOWCASE_FLOAT_OFFSET_PX,
};

pub const SHOWCASE_OUTPUT: &str = "widget-showcase";
const TERMINAL_PADDING_PX: f32 = 10.0;
const WINDOW_CORNER_RADIUS_PX: f32 = 8.0;
const FOCUS_RING_WIDTH_PX: f32 = 3.0;

fn configured_in_fixture_order(snapshot: &Snapshot) -> ConfiguredWorkspaces {
    snapshot
        .workspaces_for_output(SHOWCASE_OUTPUT)
        .iter()
        .filter_map(|p| p.name.clone())
        .collect()
}

#[must_use]
pub fn recorded_look() -> ShellAppearance {
    AppearancePreset::ClearDark.applied_to(ShellAppearance::default())
}

fn undragged_slot_context(
    config: &Config,
    edge: Position,
    clock: Clock,
    window_row_layout: WindowRowLayout,
) -> PanelSlotContext<'_> {
    let panel_appearance = recorded_look().panels;
    PanelSlotContext {
        config,
        edge,
        drag: DragView {
            drag: None,
            ghost_frame: panel_appearance.frame,
        },
        clock,
        panel_corner_radius_px: panel_appearance.frame.corner_radius_px,
        panel_float_offset_px: 0.0,
        panel_frame_style: panel_appearance.frame,
        window_row_layout,
        window_title_length: WindowTitleLength::default(),
    }
}

pub enum ShowcasedGroup {
    Compute(Box<ComputeGroup>),
    MemoryBoard(Box<MemoryBoardGroup>),
    Weather(Box<WeatherGroup>),
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

pub struct ShowcasedShellSettings {
    pub panels: PanelSettingsGroup,
    pub sidebar: SidebarSettingsGroup,
    pub overlays: OverlaySettingsGroup,
    pub menus: GroupMenuState,
    pub compositor_styling: CompositorStylingGroup,
    pub shown_items: ShownSettingItems,
}

#[derive(Clone, Copy)]
pub enum ShownSettingItems {
    SharingAUnit,
    Remaining,
}

impl ShowcasedShellSettings {
    fn group_contents<'a>(
        &'a self,
        config: &'a Config,
    ) -> Vec<WidgetGroupContent<'a, ShowcaseInteraction>> {
        let inert_settings = [
            self.panels
                .group_content(config)
                .map(|_| ShowcaseInteraction::OutsideShowcase),
            self.sidebar
                .group_content(config)
                .map(|_| ShowcaseInteraction::OutsideShowcase),
            self.overlays
                .group_content(config)
                .map(|_| ShowcaseInteraction::OutsideShowcase),
            self.compositor_styling
                .group_content(config)
                .map(|_| ShowcaseInteraction::OutsideShowcase),
            self.menus
                .settings_content(config)
                .map(|_| ShowcaseInteraction::OutsideShowcase),
        ];
        match self.shown_items {
            ShownSettingItems::SharingAUnit => inert_settings
                .map(WidgetGroupContent::only_aligned_items)
                .into(),
            ShownSettingItems::Remaining => inert_settings
                .map(WidgetGroupContent::only_unaligned_items)
                .into(),
        }
    }
}

pub struct ShowcasedAppLauncher {
    pub group: AppLauncherGroup,
    pub desktop_entries: DesktopIndex,
}

pub struct ShowcasedGamepad {
    pub group: GamepadGroup,
    pub bindings: GamepadBindingsConfig,
}

pub struct ShowcasedToasts {
    pub compositor: CompositorWindowState,
    pub opening: Vec<NotificationData>,
    pub terminal_scrollback: &'static [TerminalLine],
}

pub enum TerminalLine {
    Prompt {
        directory: &'static str,
        command: &'static str,
    },
    Status {
        verb: &'static str,
        detail: &'static str,
    },
}

impl TerminalLine {
    fn view<'a>(&self) -> Element<'a, ShowcaseInteraction> {
        match self {
            TerminalLine::Prompt { directory, command } => {
                terminal_row(directory, primary_color, command)
            }
            TerminalLine::Status { verb, detail } => terminal_row(verb, success_color, detail),
        }
    }
}

fn terminal_row<'a>(
    lead: &'static str,
    lead_color: fn(&Theme) -> Color,
    rest: &'static str,
) -> Element<'a, ShowcaseInteraction> {
    Row::with_children([
        text(lead)
            .font(Font::MONOSPACE)
            .style(move |theme| text::Style {
                color: Some(lead_color(theme)),
            })
            .into(),
        text(rest).font(Font::MONOSPACE).style(text::base).into(),
    ])
    .into()
}

impl ShowcasedToasts {
    fn window_frames(&self) -> Element<'_, ShowcaseInteraction> {
        let frames = self.compositor.windows.values().filter_map(|window| {
            let (x, y) = window.layout.tile_position_in_view?;
            let (width, height) = window.layout.window_size;
            let focus_ring: fn(&Theme) -> Color = if window.is_focused {
                primary_color
            } else {
                secondary_text_color
            };
            let scrollback =
                Column::with_children(self.terminal_scrollback.iter().map(TerminalLine::view));
            let frame = container(scrollback)
                .padding(TERMINAL_PADDING_PX)
                .width(Length::Fixed(f32::from(
                    u16::try_from(width).expect("fixture window widths fit in u16"),
                )))
                .height(Length::Fixed(f32::from(
                    u16::try_from(height).expect("fixture window heights fit in u16"),
                )))
                .style(move |theme| container::Style {
                    background: Some(background_color(theme).into()),
                    border: border::rounded(WINDOW_CORNER_RADIUS_PX)
                        .color(focus_ring(theme))
                        .width(FOCUS_RING_WIDTH_PX),
                    ..container::Style::default()
                });
            Some(
                container(frame)
                    .padding(Padding::ZERO.top(y as f32).left(x as f32))
                    .into(),
            )
        });
        stack(frames).into()
    }
}

pub struct ShowcasedMedia {
    pub group: MediaGroup,
    pub desktop_entries: DesktopIndex,
}

pub struct ShowcasedDesktopSession {
    pub window_group: WindowGroup,
    pub window_row_layout: WindowRowLayout,
    pub compositor: CompositorWindowState,
    pub snapshot: Snapshot,
    pub desktop_entries: DesktopIndex,
    pub favicons: Favicons,
    pub controller_artwork: ControllerArtwork,
}

impl ShowcasedDesktopSession {
    fn sidebar_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseInteraction>> {
        sidebar_column::vertical_view(
            &self.snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(&self.snapshot)),
            &EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown),
            recorded_look().sidebar.frame.drawn_corners(
                outlined_sides_inward_of(Position::Left)
                    .all_when_floating(SHOWCASE_FLOAT_OFFSET_PX),
            ),
            undragged_slot_context(config, Position::Left, clock, self.window_row_layout),
        )
        .map(|workspaces| workspaces.map(ShowcaseInteraction::from))
        .into_iter()
        .collect()
    }

    fn workspaces_and_windows<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseInteraction>> {
        let snapshot = &self.snapshot;
        let workspaces = inline_strip::inline_view(
            snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(snapshot)),
            &EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown),
            undragged_slot_context(config, Position::Bottom, clock, self.window_row_layout),
        )
        .map(|workspaces| workspaces.map(ShowcaseInteraction::from));
        let windows = self
            .window_group
            .view(
                snapshot,
                WindowAssets {
                    entries: &self.desktop_entries,
                    favicons: &self.favicons,
                    controller_artwork: &self.controller_artwork,
                },
                None,
                SHOWCASE_OUTPUT,
                undragged_slot_context(config, Position::Bottom, clock, self.window_row_layout),
            )
            .map(|element| {
                element.map(|event| {
                    ShowcaseInteraction::from(event.map(ShowcaseInteraction::WindowGroup))
                })
            });
        workspaces.into_iter().chain(windows).collect()
    }

    fn standalone_hover_card(
        &self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Vec<Section> {
        let silent_windows = HashMap::new();
        let window_press_actions = |window: u64| {
            self.window_group
                .window_press_actions(&self.snapshot, &silent_windows, window, config)
        };
        match target {
            HoverTarget::Window(WindowTarget::Usage(window)) => {
                WindowGroup::window_identity_hover_card(&self.compositor, *window, clock)
                    .into_iter()
                    .chain(Section::press_actions(window_press_actions(*window)))
                    .collect()
            }
            HoverTarget::Window(WindowTarget::Inspect(window)) => self
                .window_group
                .inspect_hover_card()
                .into_iter()
                .chain(Section::press_actions(window_press_actions(*window)))
                .collect(),
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.snapshot,
                &self.desktop_entries,
                AppWindowLayout::default(),
                *workspace,
            ),
            _ => Vec::new(),
        }
    }
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
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::VoiceDictation(_)
            | ShowcasedGroup::Caldav(_)
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

    pub fn group_contents<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<WidgetGroupContent<'a, ShowcaseInteraction>> {
        let locale = config.calendar.locale;
        match self {
            ShowcasedGroup::AppLauncher(launcher) => launcher
                .group
                .group_contents(&launcher.desktop_entries, config, SHOWCASE_OUTPUT)
                .into_iter()
                .map(|group| group.map(|_| ShowcaseInteraction::OutsideShowcase))
                .collect(),
            ShowcasedGroup::Compute(group) => {
                vec![
                    group
                        .group_content(clock, locale)
                        .map(ShowcaseInteraction::Compute),
                ]
            }
            ShowcasedGroup::MemoryBoard(group) => {
                vec![
                    group
                        .group_content(clock, locale)
                        .map(ShowcaseInteraction::MemoryBoard),
                ]
            }
            ShowcasedGroup::Weather(group) => {
                vec![
                    group
                        .group_content(config, clock)
                        .map(ShowcaseInteraction::Weather),
                ]
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                vec![
                    group
                        .group_content(clock)
                        .map(ShowcaseInteraction::ClaudeUsage),
                ]
            }
            ShowcasedGroup::Media(media) => media
                .group
                .group_contents(config, &media.desktop_entries, clock)
                .map(|group| group.map(ShowcaseInteraction::Media))
                .collect(),
            ShowcasedGroup::IdleTimeouts(group) => group
                .pause_groups(config, clock)
                .into_iter()
                .map(|pause_group| pause_group.map(ShowcaseInteraction::IdleTimeouts))
                .collect(),
            ShowcasedGroup::Wallpaper(group) => {
                vec![
                    group
                        .group_content(clock)
                        .map(ShowcaseInteraction::Wallpaper),
                ]
            }
            ShowcasedGroup::Audio(group) => {
                vec![group.group_content(config).map(ShowcaseInteraction::Audio)]
            }
            ShowcasedGroup::Gamepad(gamepad) => gamepad
                .group
                .group_contents(&gamepad.bindings)
                .into_iter()
                .map(|group| group.map(ShowcaseInteraction::Gamepad))
                .collect(),
            ShowcasedGroup::VoiceDictation(group) => {
                vec![
                    group
                        .group_content(clock)
                        .map(ShowcaseInteraction::VoiceDictation),
                ]
            }
            ShowcasedGroup::PodmanEngines(_)
            | ShowcasedGroup::VirtualMachines(_)
            | ShowcasedGroup::NspawnContainers(_)
            | ShowcasedGroup::DockerEngines(_)
            | ShowcasedGroup::ApptainerContainers(_)
            | ShowcasedGroup::VmwareMachines(_)
            | ShowcasedGroup::VirtualBoxMachines(_)
            | ShowcasedGroup::WaydroidContainer(_)
            | ShowcasedGroup::FlatpakSandboxes(_)
            | ShowcasedGroup::LxcContainers(_)
            | ShowcasedGroup::IncusInstances(_)
            | ShowcasedGroup::MicrovmGuests(_)
            | ShowcasedGroup::CgroupMemoryLimits(_)
            | ShowcasedGroup::CiRuns(_) => self.guest_group_contents(config, clock),
            ShowcasedGroup::Caldav(group) => {
                vec![
                    group
                        .group_content(&config.widget_groups.caldav, clock, locale)
                        .map(ShowcaseInteraction::Caldav),
                ]
            }
            ShowcasedGroup::Tailnet(group) => {
                vec![group.group_content(clock).map(ShowcaseInteraction::Tailnet)]
            }
            ShowcasedGroup::WireguardTunnels(group) => {
                vec![
                    group
                        .group_content()
                        .map(ShowcaseInteraction::WireguardTunnels),
                ]
            }
            ShowcasedGroup::LoginSession(group) => {
                vec![group.group_content().map(ShowcaseInteraction::LoginSession)]
            }
            ShowcasedGroup::MenuMembers(groups) => groups
                .iter()
                .flat_map(|member| member.group_contents(config, clock))
                .collect(),
            ShowcasedGroup::ShellSettings(settings) => settings.group_contents(config),
            ShowcasedGroup::AppearancePresets(presets) => vec![
                presets
                    .group
                    .group_content(&presets.appearance)
                    .map(ShowcaseInteraction::AppearancePresets),
            ],
            ShowcasedGroup::BottomPanel(_)
            | ShowcasedGroup::WorkspaceSidebar(_)
            | ShowcasedGroup::Toasts(_) => Vec::new(),
        }
    }

    fn guest_group_contents<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<WidgetGroupContent<'a, ShowcaseInteraction>> {
        let locale = config.calendar.locale;
        let guest_group = match self {
            ShowcasedGroup::PodmanEngines(group) => group
                .group_content()
                .map(ShowcaseInteraction::PodmanEngines),
            ShowcasedGroup::VirtualMachines(group) => group
                .group_content()
                .map(ShowcaseInteraction::VirtualMachines),
            ShowcasedGroup::NspawnContainers(group) => group
                .group_content()
                .map(ShowcaseInteraction::NspawnContainers),
            ShowcasedGroup::DockerEngines(group) => group
                .group_content()
                .map(ShowcaseInteraction::DockerEngines),
            ShowcasedGroup::ApptainerContainers(group) => group
                .group_content()
                .map(ShowcaseInteraction::ApptainerContainers),
            ShowcasedGroup::VmwareMachines(group) => group
                .group_content()
                .map(ShowcaseInteraction::VmwareMachines),
            ShowcasedGroup::VirtualBoxMachines(group) => group
                .group_content()
                .map(ShowcaseInteraction::VirtualBoxMachines),
            ShowcasedGroup::WaydroidContainer(group) => group
                .group_content()
                .map(ShowcaseInteraction::WaydroidContainer),
            ShowcasedGroup::FlatpakSandboxes(group) => group
                .group_content()
                .map(ShowcaseInteraction::FlatpakSandboxes),
            ShowcasedGroup::LxcContainers(group) => group
                .group_content()
                .map(ShowcaseInteraction::LxcContainers),
            ShowcasedGroup::IncusInstances(group) => group
                .group_content()
                .map(ShowcaseInteraction::IncusInstances),
            ShowcasedGroup::MicrovmGuests(group) => group
                .group_content()
                .map(ShowcaseInteraction::MicrovmGuests),
            ShowcasedGroup::CgroupMemoryLimits(group) => group
                .group_content(config)
                .map(ShowcaseInteraction::CgroupMemoryLimits),
            ShowcasedGroup::CiRuns(group) => group
                .group_content(clock, locale)
                .map(ShowcaseInteraction::CiRuns),
            _ => return Vec::new(),
        };
        vec![guest_group]
    }

    pub fn panel_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseInteraction>> {
        let position = self.position();
        match self {
            ShowcasedGroup::BottomPanel(session) => session.workspaces_and_windows(config, clock),
            ShowcasedGroup::WorkspaceSidebar(session) => session.sidebar_groups(config, clock),
            _ => self
                .group_contents(config, clock)
                .into_iter()
                .filter_map(|group| group.view(config, position, clock))
                .collect(),
        }
    }

    fn update_paired_group(
        &mut self,
        message: ShowcaseMessage,
        config: &Config,
        clock: Clock,
    ) -> Result<(), Box<ShowcaseMessage>> {
        match (self, message) {
            (ShowcasedGroup::PodmanEngines(group), ShowcaseMessage::PodmanEngines(message)) => {
                let _ = group.update(message, clock);
            }
            (ShowcasedGroup::VirtualMachines(group), ShowcaseMessage::VirtualMachines(message)) => {
                let _ = group.update(message, clock);
            }
            (
                ShowcasedGroup::NspawnContainers(group),
                ShowcaseMessage::NspawnContainers(container_message),
            ) => {
                let _ = group.update(container_message, clock);
            }
            (ShowcasedGroup::CiRuns(group), ShowcaseMessage::CiRuns(message)) => {
                let _ = group.update(message, clock);
            }
            (ShowcasedGroup::DockerEngines(group), ShowcaseMessage::DockerEngines(message)) => {
                let _ = group.update(message, clock);
            }
            (
                ShowcasedGroup::ApptainerContainers(group),
                ShowcaseMessage::ApptainerContainers(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::VmwareMachines(group),
                ShowcaseMessage::VmwareMachines(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::VirtualBoxMachines(group),
                ShowcaseMessage::VirtualBoxMachines(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::WaydroidContainer(group),
                ShowcaseMessage::WaydroidContainer(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::FlatpakSandboxes(group),
                ShowcaseMessage::FlatpakSandboxes(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::LxcContainers(group),
                ShowcaseMessage::LxcContainers(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::IncusInstances(group),
                ShowcaseMessage::IncusInstances(group_message),
            ) => {
                let _ = group.update(group_message, clock);
            }
            (
                ShowcasedGroup::MicrovmGuests(group),
                ShowcaseMessage::MicrovmGuests(microvm_message),
            ) => {
                let _ = group.update(microvm_message, clock);
            }
            (
                ShowcasedGroup::CgroupMemoryLimits(group),
                ShowcaseMessage::CgroupMemoryLimits(limits_message),
            ) => {
                let _ = group.update(limits_message, clock);
            }
            (ShowcasedGroup::Caldav(group), ShowcaseMessage::Caldav(message)) => {
                let _ = group.update(message, clock);
            }
            (
                ShowcasedGroup::WireguardTunnels(group),
                ShowcaseMessage::WireguardTunnels(message),
            ) => {
                let _ = group.update(message, clock);
            }
            (ShowcasedGroup::Tailnet(group), ShowcaseMessage::Tailnet(message)) => {
                let _ = group.update(message, config, clock);
            }
            (ShowcasedGroup::LoginSession(group), ShowcaseMessage::LoginSession(message)) => {
                let _ = group.update(message, config, clock);
            }
            (_, unclaimed) => return Err(Box::new(unclaimed)),
        }
        Ok(())
    }

    pub fn update(
        &mut self,
        message: ShowcaseMessage,
        config: &Config,
        clock: Clock,
    ) -> Result<(), Box<ShowcaseMessage>> {
        match self {
            ShowcasedGroup::Compute(group) => {
                if let ShowcaseMessage::Compute(message) = message {
                    let _ = group.update(message, config, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::MemoryBoard(group) => {
                if let ShowcaseMessage::MemoryBoard(message) = message {
                    let _ = group.update(message, config, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::Weather(group) => {
                if let ShowcaseMessage::Weather(message) = message {
                    let _ = group.update(message, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                if let ShowcaseMessage::ClaudeUsage(message) = message {
                    let _ = group.update(message, config, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::Media(media) => {
                if let ShowcaseMessage::Media(message) = message {
                    let _ = media.group.update(message, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::IdleTimeouts(group) => {
                if let ShowcaseMessage::IdleTimeouts(message) = message {
                    let _ = group.update(message, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::Wallpaper(group) => {
                if let ShowcaseMessage::Wallpaper(message) = message {
                    let _ = group.update(message, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::Audio(group) => {
                if let ShowcaseMessage::Audio(message) = message {
                    let _ = group.update(message, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::Gamepad(gamepad) => {
                if let ShowcaseMessage::Gamepad(message) = message {
                    let _ = gamepad.group.update(message, &gamepad.bindings);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::VoiceDictation(group) => {
                if let ShowcaseMessage::VoiceDictation(message) = message {
                    let _ = group.update(message, config, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::PodmanEngines(_)
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
            | ShowcasedGroup::Caldav(_)
            | ShowcasedGroup::WireguardTunnels(_)
            | ShowcasedGroup::Tailnet(_)
            | ShowcasedGroup::LoginSession(_) => {
                return self.update_paired_group(message, config, clock);
            }
            ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                if let ShowcaseMessage::WindowGroup(message) = message {
                    let _ = session.window_group.update(message, clock);
                } else {
                    return Err(Box::new(message));
                }
            }
            ShowcasedGroup::MenuMembers(groups) => {
                return groups
                    .iter_mut()
                    .try_fold(message, |unclaimed, member| {
                        match member.update(unclaimed, config, clock) {
                            Ok(()) => ControlFlow::Break(()),
                            Err(still_unclaimed) => ControlFlow::Continue(*still_unclaimed),
                        }
                    })
                    .continue_value()
                    .map_or(Ok(()), |unclaimed| Err(Box::new(unclaimed)));
            }
            ShowcasedGroup::AppearancePresets(presets) => match message {
                ShowcaseMessage::AppearancePresets(AppearancePresetsMessage::Apply(preset)) => {
                    presets.appearance = preset.applied_to(presets.appearance);
                }
                ShowcaseMessage::AppearancePresets(AppearancePresetsMessage::Expansion(
                    expansion,
                )) => presets.group.update_expansion(expansion),
                unclaimed => return Err(Box::new(unclaimed)),
            },
            ShowcasedGroup::AppLauncher(_)
            | ShowcasedGroup::Toasts(_)
            | ShowcasedGroup::ShellSettings(_) => return Err(Box::new(message)),
        }
        Ok(())
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
            let _watched_window = session.window_group.follow_window_inspection(
                &session.compositor,
                WindowInspection::from(Some(target)),
                config,
                clock,
            );
            return desktop_session::on_hover(target)
                .into_iter()
                .map(ShowcaseMessage::WindowGroup)
                .collect();
        }
        let hovered_fetch = self
            .group_contents(config, clock)
            .iter()
            .find_map(|group| group.hovered_instrument(target))
            .map(|hovered_row| hovered_row.hover_fetch().clone());
        let Some(HoverFetch::Fetch { start, .. }) = hovered_fetch else {
            return Vec::new();
        };
        let started_fetch = ShowcaseMessage::from(start);
        let fetch_replies = hover_fetch_replies(&started_fetch);
        let _started = self.update(started_fetch, config, clock);
        fetch_replies
    }

    pub fn described_hover_card(
        &self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Option<HoverCard> {
        let row_card = self
            .group_contents(config, clock)
            .iter()
            .find_map(|group| group.hover_card(target, clock));
        if let Some(row_card) = row_card {
            return HoverCard::new(row_card.sections, row_card.header);
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
            | ShowcasedGroup::ClaudeUsage(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::VoiceDictation(_)
            | ShowcasedGroup::Caldav(_)
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
