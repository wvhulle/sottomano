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
    Config,
    screen_edges::Position,
    widget_groups::{caldav_account::CaldavAccountConfig, gamepad_bindings::GamepadBindingsConfig},
};
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    desktop_panels::{panel_frame::outlined_sides_inward_of, panel_slot_drag::PanelSlotContext},
    group_menus::group_menu_state::GroupMenuState,
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        app_launcher_group::AppLauncherGroup,
        appearance_presets_group::{AppearancePresetsGroup, AppearancePresetsMessage},
        audio_group::AudioGroup,
        caldav_group::CaldavGroup,
        ci_runs_group::CiRunsGroup,
        claude_usage_group::ClaudeUsageGroup,
        compositor_styling_group::CompositorStylingGroup,
        compute_group::ComputeGroup,
        gamepad_group::GamepadGroup,
        idle_pause_groups::IdlePauseGroups,
        login_session_group::LoginSessionGroup,
        media_group::MediaGroup,
        memory_board_group::MemoryBoardGroup,
        nspawn_containers_group::NspawnContainersGroup,
        overlay_settings_group::OverlaySettingsGroup,
        panel_settings_group::PanelSettingsGroup,
        podman_engines_group::PodmanEnginesGroup,
        sidebar_settings_group::SidebarSettingsGroup,
        tailnet_group::TailnetGroup,
        virtual_machines_group::VirtualMachinesGroup,
        voice_dictation_group::VoiceDictationGroup,
        wallpaper_group::WallpaperGroup,
        weather_group::WeatherGroup,
        widget_group_content::WidgetGroupContent,
        window_group::{WindowGroup, WindowRowLayout, WindowTarget},
        wireguard_tunnels_group::WireguardTunnelsGroup,
        workspaces_group::{
            EmptyWorkspaceFolding, EmptyWorkspaces, inline_strip, sidebar_column,
            workspace_hover_card,
        },
    },
    widgets::{
        appearance_presets::{AppearancePreset, ShellAppearance},
        configured_theme::{background_color, primary_color, secondary_text_color, success_color},
        favicon::Favicons,
        pointer_drag::DragView,
        window_icon::WindowAssets,
    },
};

use crate::{
    fixtures::{
        claude_usage, compute, desktop_session, login_session, memory_board, tailnet,
        voice_dictation, wireguard_tunnels,
    },
    showcase_message::ShowcaseMessage,
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
            ghost_frame: panel_appearance.frame_style(),
        },
        clock,
        panel_corner_radius_px: panel_appearance.corner_radius_px,
        panel_frame_style: panel_appearance.frame_style(),
        window_row_layout,
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
    ) -> Vec<WidgetGroupContent<'a, ShowcaseMessage>> {
        let inert_settings = [
            self.panels
                .group_content(config)
                .map(|_| ShowcaseMessage::OutsideShowcase),
            self.sidebar
                .group_content(config)
                .map(|_| ShowcaseMessage::OutsideShowcase),
            self.overlays
                .group_content(config)
                .map(|_| ShowcaseMessage::OutsideShowcase),
            self.compositor_styling
                .group_content(config)
                .map(|_| ShowcaseMessage::OutsideShowcase),
            self.menus
                .settings_content(config)
                .map(|_| ShowcaseMessage::OutsideShowcase),
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

    fn hover_card_for(&self, target: &HoverTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            HoverTarget::PanelSettings(target) => Some(self.panels.hover_card_for(*target, clock)),
            HoverTarget::SidebarSettings(target) => {
                Some(self.sidebar.hover_card_for(*target, clock))
            }
            HoverTarget::OverlaySettings(target) => {
                Some(self.overlays.hover_card_for(*target, clock))
            }
            HoverTarget::CompositorStyling(target) => {
                Some(self.compositor_styling.hover_card_for(*target, clock))
            }
            HoverTarget::GroupMenuSettings(target) => {
                Some(self.menus.settings_hover_card(*target, clock))
            }
            _ => None,
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
    fn view<'a>(&self) -> Element<'a, ShowcaseMessage> {
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
) -> Element<'a, ShowcaseMessage> {
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
    fn window_frames(&self) -> Element<'_, ShowcaseMessage> {
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
                .width(Length::Fixed(f32::from(u16::try_from(width).ok()?)))
                .height(Length::Fixed(f32::from(u16::try_from(height).ok()?)))
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
    ) -> Vec<Element<'a, ShowcaseMessage>> {
        sidebar_column::vertical_view(
            &self.snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(&self.snapshot)),
            &EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown),
            recorded_look().sidebar.frame_style().drawn_corners(
                outlined_sides_inward_of(Position::Left)
                    .all_when_floating(SHOWCASE_FLOAT_OFFSET_PX),
            ),
            undragged_slot_context(config, Position::Left, clock, self.window_row_layout),
        )
        .map(|workspaces| workspaces.map(ShowcaseMessage::from))
        .into_iter()
        .collect()
    }

    fn workspaces_and_windows<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseMessage>> {
        let snapshot = &self.snapshot;
        let workspaces = inline_strip::inline_view(
            snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(snapshot)),
            &EmptyWorkspaceFolding::new(EmptyWorkspaces::Shown),
            undragged_slot_context(config, Position::Bottom, clock, self.window_row_layout),
        )
        .map(|workspaces| workspaces.map(ShowcaseMessage::from));
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
                element.map(|event| ShowcaseMessage::from(event.map(ShowcaseMessage::WindowGroup)))
            });
        workspaces.into_iter().chain(windows).collect()
    }

    fn hover_card_for(&self, target: &HoverTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_hover_card(&self.compositor, *window, clock)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                Some(self.window_group.inspect_hover_card(clock))
            }
            HoverTarget::Workspace(workspace) => workspace_hover_card::hover_card(
                &self.snapshot,
                &self.desktop_entries,
                AppWindowLayout::default(),
                *workspace,
            ),
            _ => None,
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
    ) -> Vec<WidgetGroupContent<'a, ShowcaseMessage>> {
        match self {
            ShowcasedGroup::AppLauncher(launcher) => launcher
                .group
                .group_contents(&launcher.desktop_entries, config, SHOWCASE_OUTPUT)
                .into_iter()
                .map(|group| group.map(|_| ShowcaseMessage::OutsideShowcase))
                .collect(),
            ShowcasedGroup::Compute(group) => {
                vec![group.group_content().map(ShowcaseMessage::Compute)]
            }
            ShowcasedGroup::MemoryBoard(group) => {
                vec![group.group_content().map(ShowcaseMessage::MemoryBoard)]
            }
            ShowcasedGroup::Weather(group) => {
                vec![group.group_content(config).map(ShowcaseMessage::Weather)]
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                vec![group.group_content().map(ShowcaseMessage::ClaudeUsage)]
            }
            ShowcasedGroup::Media(media) => media
                .group
                .group_contents(config, clock)
                .map(|group| group.map(ShowcaseMessage::Media))
                .collect(),
            ShowcasedGroup::IdleTimeouts(group) => group
                .pause_groups(config)
                .into_iter()
                .map(|pause_group| pause_group.map(ShowcaseMessage::IdleTimeouts))
                .collect(),
            ShowcasedGroup::Wallpaper(group) => {
                vec![group.group_content().map(ShowcaseMessage::Wallpaper)]
            }
            ShowcasedGroup::Audio(group) => {
                vec![group.group_content(config).map(ShowcaseMessage::Audio)]
            }
            ShowcasedGroup::Gamepad(gamepad) => gamepad
                .group
                .group_contents(&gamepad.bindings)
                .into_iter()
                .map(|group| group.map(ShowcaseMessage::Gamepad))
                .collect(),
            ShowcasedGroup::VoiceDictation(group) => {
                vec![group.group_content().map(ShowcaseMessage::VoiceDictation)]
            }
            ShowcasedGroup::PodmanEngines(group) => {
                vec![group.group_content().map(ShowcaseMessage::PodmanEngines)]
            }
            ShowcasedGroup::VirtualMachines(group) => {
                vec![group.group_content().map(ShowcaseMessage::VirtualMachines)]
            }
            ShowcasedGroup::NspawnContainers(group) => {
                vec![group.group_content().map(ShowcaseMessage::NspawnContainers)]
            }
            ShowcasedGroup::CiRuns(group) => {
                vec![group.group_content().map(ShowcaseMessage::CiRuns)]
            }
            ShowcasedGroup::Caldav(group) => {
                vec![
                    group
                        .group_content(&config.widget_groups.caldav)
                        .map(ShowcaseMessage::Caldav),
                ]
            }
            ShowcasedGroup::Tailnet(group) => {
                vec![group.group_content().map(ShowcaseMessage::Tailnet)]
            }
            ShowcasedGroup::WireguardTunnels(group) => {
                vec![group.group_content().map(ShowcaseMessage::WireguardTunnels)]
            }
            ShowcasedGroup::LoginSession(group) => {
                vec![group.group_content().map(ShowcaseMessage::LoginSession)]
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
                    .map(ShowcaseMessage::AppearancePresets),
            ],
            ShowcasedGroup::BottomPanel(_)
            | ShowcasedGroup::WorkspaceSidebar(_)
            | ShowcasedGroup::Toasts(_) => Vec::new(),
        }
    }

    pub fn panel_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseMessage>> {
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

    fn update_container_groups(&mut self, message: ShowcaseMessage, clock: Clock) {
        match (self, message) {
            (ShowcasedGroup::PodmanEngines(group), ShowcaseMessage::PodmanEngines(message)) => {
                group.update(message, clock);
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
            _ => {}
        }
    }

    pub fn update(&mut self, message: ShowcaseMessage, clock: Clock) {
        match self {
            ShowcasedGroup::Compute(group) => {
                if let ShowcaseMessage::Compute(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::MemoryBoard(group) => {
                if let ShowcaseMessage::MemoryBoard(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Weather(group) => {
                if let ShowcaseMessage::Weather(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                if let ShowcaseMessage::ClaudeUsage(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Media(media) => {
                if let ShowcaseMessage::Media(message) = message {
                    let _ = media.group.update(message, clock);
                }
            }
            ShowcasedGroup::IdleTimeouts(group) => {
                if let ShowcaseMessage::IdleTimeouts(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Wallpaper(group) => {
                if let ShowcaseMessage::Wallpaper(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Audio(group) => {
                if let ShowcaseMessage::Audio(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Gamepad(gamepad) => {
                if let ShowcaseMessage::Gamepad(message) = message {
                    let _ = gamepad.group.update(message, &gamepad.bindings);
                }
            }
            ShowcasedGroup::VoiceDictation(group) => {
                if let ShowcaseMessage::VoiceDictation(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::PodmanEngines(_)
            | ShowcasedGroup::VirtualMachines(_)
            | ShowcasedGroup::NspawnContainers(_)
            | ShowcasedGroup::CiRuns(_) => self.update_container_groups(message, clock),
            ShowcasedGroup::Caldav(group) => {
                if let ShowcaseMessage::Caldav(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::WireguardTunnels(group) => {
                if let ShowcaseMessage::WireguardTunnels(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Tailnet(group) => {
                if let ShowcaseMessage::Tailnet(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::LoginSession(group) => {
                if let ShowcaseMessage::LoginSession(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                if let ShowcaseMessage::WindowGroup(message) = message {
                    let _ = session.window_group.update(message, clock);
                }
            }
            ShowcasedGroup::MenuMembers(groups) => {
                for member in groups {
                    member.update(message.clone(), clock);
                }
            }
            ShowcasedGroup::AppearancePresets(presets) => match message {
                ShowcaseMessage::AppearancePresets(AppearancePresetsMessage::Apply(preset)) => {
                    presets.appearance = preset.applied_to(presets.appearance);
                }
                ShowcaseMessage::AppearancePresets(AppearancePresetsMessage::Expansion(
                    expansion,
                )) => presets.group.update_expansion(expansion),
                _ => {}
            },
            ShowcasedGroup::AppLauncher(_)
            | ShowcasedGroup::Toasts(_)
            | ShowcasedGroup::ShellSettings(_) => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn hover(
        &mut self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Vec<ShowcaseMessage> {
        match self {
            ShowcasedGroup::Compute(group) => {
                let _ = group.hover(Some(target), config, clock);
                let HoverTarget::Compute(target) = target else {
                    return Vec::new();
                };
                compute::on_hover(*target)
                    .into_iter()
                    .map(ShowcaseMessage::Compute)
                    .collect()
            }
            ShowcasedGroup::MemoryBoard(group) => {
                let _ = group.hover(Some(target), config, clock);
                let HoverTarget::MemoryBoard(target) = target else {
                    return Vec::new();
                };
                memory_board::on_hover(target)
                    .into_iter()
                    .map(ShowcaseMessage::MemoryBoard)
                    .collect()
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                let _ = group.hover(Some(target), config, None, clock);
                let HoverTarget::ClaudeUsage(target) = target else {
                    return Vec::new();
                };
                claude_usage::on_hover(*target, clock)
                    .into_iter()
                    .map(ShowcaseMessage::ClaudeUsage)
                    .collect()
            }
            ShowcasedGroup::VoiceDictation(group) => {
                let _ = group.hover(Some(target), config, clock);
                let HoverTarget::VoiceDictation(target) = target else {
                    return Vec::new();
                };
                voice_dictation::on_hover(*target)
                    .into_iter()
                    .map(ShowcaseMessage::VoiceDictation)
                    .collect()
            }
            ShowcasedGroup::WireguardTunnels(group) => {
                let _ = group.hover(Some(target), config, None, clock);
                let HoverTarget::WireguardTunnel(target) = target else {
                    return Vec::new();
                };
                wireguard_tunnels::on_hover(target)
                    .into_iter()
                    .map(ShowcaseMessage::WireguardTunnels)
                    .collect()
            }
            ShowcasedGroup::Tailnet(group) => {
                let _ = group.hover(Some(target), config, None, clock);
                let HoverTarget::Tailnet(target) = target else {
                    return Vec::new();
                };
                tailnet::on_hover(target)
                    .into_iter()
                    .map(ShowcaseMessage::Tailnet)
                    .collect()
            }
            ShowcasedGroup::LoginSession(group) => {
                let _ = group.hover(Some(target), config, None, clock);
                let HoverTarget::LoginSession(target) = target else {
                    return Vec::new();
                };
                login_session::on_hover(*target, clock)
                    .into_iter()
                    .map(ShowcaseMessage::LoginSession)
                    .collect()
            }
            ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                let _ =
                    session
                        .window_group
                        .hover(&session.compositor, Some(target), config, clock);
                desktop_session::on_hover(target)
                    .into_iter()
                    .map(ShowcaseMessage::WindowGroup)
                    .collect()
            }
            ShowcasedGroup::MenuMembers(groups) => groups
                .iter_mut()
                .flat_map(|member| member.hover(target, config, clock))
                .collect(),
            ShowcasedGroup::AppLauncher(_)
            | ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::Caldav(_)
            | ShowcasedGroup::PodmanEngines(_)
            | ShowcasedGroup::VirtualMachines(_)
            | ShowcasedGroup::NspawnContainers(_)
            | ShowcasedGroup::CiRuns(_)
            | ShowcasedGroup::AppearancePresets(_)
            | ShowcasedGroup::ShellSettings(_)
            | ShowcasedGroup::Toasts(_) => Vec::new(),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn hover_card_for(
        &self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Option<HoverCard> {
        match self {
            ShowcasedGroup::Compute(group) => {
                let HoverTarget::Compute(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, clock)
            }
            ShowcasedGroup::MemoryBoard(group) => {
                let HoverTarget::MemoryBoard(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::Weather(group) => {
                let HoverTarget::Weather(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, config, clock)
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                let HoverTarget::ClaudeUsage(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, clock)
            }
            ShowcasedGroup::Media(media) => {
                let HoverTarget::Media(target) = target else {
                    return None;
                };
                media
                    .group
                    .hover_card_for(target, &media.desktop_entries, clock)
            }
            ShowcasedGroup::IdleTimeouts(group) => match target {
                HoverTarget::IdleTimeout(kind) => Some(group.hover_card_for(*kind, clock)),
                HoverTarget::PauseBlocker(blocker) => group.blocker_hover_card_for(blocker),
                _ => None,
            },
            ShowcasedGroup::Wallpaper(group) => {
                let HoverTarget::Wallpaper(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, clock)
            }
            ShowcasedGroup::AppLauncher(launcher) => {
                let HoverTarget::AppLauncher(target) = target else {
                    return None;
                };
                Some(launcher.group.hover_card_for(target, clock))
            }
            ShowcasedGroup::Audio(group) => {
                let HoverTarget::Audio(kind) = target else {
                    return None;
                };
                group.hover_card_for(*kind, clock)
            }
            ShowcasedGroup::Gamepad(gamepad) => {
                let HoverTarget::Gamepad(target) = target else {
                    return None;
                };
                gamepad
                    .group
                    .hover_card_for(*target, &gamepad.bindings, clock)
            }
            ShowcasedGroup::VoiceDictation(group) => {
                let HoverTarget::VoiceDictation(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, clock)
            }
            ShowcasedGroup::PodmanEngines(group) => {
                let HoverTarget::PodmanEngine(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::VirtualMachines(group) => {
                let HoverTarget::VirtualMachine(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::NspawnContainers(group) => {
                let HoverTarget::NspawnContainer(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::CiRuns(group) => {
                let HoverTarget::CiRuns(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::Caldav(group) => {
                let HoverTarget::Caldav(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, &CaldavAccountConfig::default(), clock)
            }
            ShowcasedGroup::WireguardTunnels(group) => {
                let HoverTarget::WireguardTunnel(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::Tailnet(group) => {
                let HoverTarget::Tailnet(target) = target else {
                    return None;
                };
                group.hover_card_for(target, clock)
            }
            ShowcasedGroup::LoginSession(group) => {
                let HoverTarget::LoginSession(target) = target else {
                    return None;
                };
                group.hover_card_for(*target, clock)
            }
            ShowcasedGroup::BottomPanel(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                session.hover_card_for(target, clock)
            }
            ShowcasedGroup::AppearancePresets(presets) => {
                let HoverTarget::AppearancePresets(target) = target else {
                    return None;
                };
                Some(presets.group.hover_card_for(*target))
            }
            ShowcasedGroup::MenuMembers(groups) => groups
                .iter()
                .find_map(|member| member.hover_card_for(target, config, clock)),
            ShowcasedGroup::ShellSettings(settings) => settings.hover_card_for(target, clock),
            ShowcasedGroup::Toasts(_) => None,
        }
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

    pub fn backdrop(&self) -> Option<Element<'_, ShowcaseMessage>> {
        match self {
            ShowcasedGroup::Toasts(toasts) => Some(toasts.window_frames()),
            _ => None,
        }
    }
}
