use iced::{
    Color, Element, Font, Length, Padding, Theme,
    border::{self, Radius},
    widget::{Column, Row, container, stack, text},
};
use iced_controller_diagram::ControllerArtwork;
use niri_ipc::state::EventStreamState;
use sottomano_config::{
    Config, screen_edges::Position, widget_groups::gamepad_bindings::GamepadBindingsConfig,
};
use sottomano_sources::{
    desktop_notifications::notification_data::NotificationData,
    desktop_windows_apps::{niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot::Snapshot},
};
use sottomano_widgets::{
    bar_slot_drag::BarSlotContext,
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::HoverTarget,
    widget_groups::{
        appearance_presets_group::{AppearancePresetsMenu, AppearancePresetsTarget},
        audio_group::AudioGroup,
        caldav_group::CaldavGroup,
        claude_usage_group::ClaudeUsageGroup,
        compute_group::ComputeGroup,
        gamepad_group::GamepadGroup,
        idle_timeouts_group::IdleTimeoutsGroup,
        login_session_group::LoginSessionGroup,
        media_group::MediaGroup,
        memory_board_group::MemoryBoardGroup,
        niri_window_group::niri_kdl_rules::NiriLayout,
        voice_dictation_group::VoiceDictationGroup,
        vpn_group::VpnGroup,
        wallpaper_group::WallpaperGroup,
        weather_group::WeatherGroup,
        window_group::{WindowGroup, WindowTarget},
        workspaces_group::{EmptyWorkspaces, inline_strip, sidebar_column, workspace_tooltip},
    },
    widgets::{
        appearance_presets::ShellAppearance,
        configured_theme::{background_color, primary_color, secondary_text_color, success_color},
        favicon::Favicons,
        panel_frame::{LayerFrameStyle, OutlinedSides},
        pointer_drag::DragView,
        shell_layer_appearance::ShellLayer,
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::GroupContent,
        window_icon::WindowAssets,
    },
};

use crate::{
    fixtures::{
        claude_usage, compute, login_session, memory_board, niri_session, voice_dictation, vpn,
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

fn undragged_slot_context(config: &Config, edge: Position, clock: Clock) -> BarSlotContext<'_> {
    BarSlotContext {
        config,
        edge,
        drag: DragView {
            drag: None,
            ghost_frame: LayerFrameStyle::new(&ShellLayer::Panels.default_appearance(), config),
        },
        clock,
    }
}

pub enum ShowcasedGroup {
    Compute(Box<ComputeGroup>),
    MemoryBoard(Box<MemoryBoardGroup>),
    Weather(Box<WeatherGroup>),
    ClaudeUsage(Box<ClaudeUsageGroup>),
    Media(Box<ShowcasedMedia>),
    IdleTimeouts(Box<IdleTimeoutsGroup>),
    AppearancePresets(AppearancePresetsMenu),
    Wallpaper(Box<WallpaperGroup>),
    Audio(Box<AudioGroup>),
    Gamepad(Box<ShowcasedGamepad>),
    VoiceDictation(Box<VoiceDictationGroup>),
    Caldav(Box<CaldavGroup>),
    Vpn(Box<VpnGroup>),
    LoginSession(Box<LoginSessionGroup>),
    BottomBar(Box<ShowcasedNiriSession>),
    WorkspaceSidebar(Box<ShowcasedNiriSession>),
    Toasts(Box<ShowcasedToasts>),
}

pub struct ShowcasedGamepad {
    pub group: GamepadGroup,
    pub bindings: GamepadBindingsConfig,
}

pub struct ShowcasedToasts {
    pub niri: EventStreamState,
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
        let frames = self.niri.windows.windows.values().filter_map(|window| {
            let (x, y) = window.layout.tile_pos_in_workspace_view?;
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

pub struct ShowcasedNiriSession {
    pub window_group: WindowGroup,
    pub niri: EventStreamState,
    pub snapshot: Snapshot,
    pub desktop_entries: DesktopIndex,
    pub favicons: Favicons,
    pub controller_artwork: ControllerArtwork,
}

impl ShowcasedNiriSession {
    fn sidebar_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseMessage>> {
        sidebar_column::vertical_view(
            &self.snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(&self.snapshot)),
            EmptyWorkspaces::Shown,
            LayerFrameStyle::new(&ShellLayer::Sidebar.default_appearance(), config).drawn_corners(
                OutlinedSides::inward_of(Position::Left)
                    .all_when_floating(SHOWCASE_FLOAT_OFFSET_PX),
            ),
            undragged_slot_context(config, Position::Left, clock),
        )
        .map(|workspaces| workspaces.map(ShowcaseMessage::from))
        .into_iter()
        .collect()
    }

    fn bottom_bar_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, ShowcaseMessage>> {
        let snapshot = &self.snapshot;
        let workspaces = inline_strip::inline_view(
            snapshot,
            SHOWCASE_OUTPUT,
            Some(&configured_in_fixture_order(snapshot)),
            EmptyWorkspaces::Shown,
            undragged_slot_context(config, Position::Bottom, clock),
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
                undragged_slot_context(config, Position::Bottom, clock),
            )
            .map(|element| {
                element.map(|event| ShowcaseMessage::from(event.map(ShowcaseMessage::WindowGroup)))
            });
        workspaces.into_iter().chain(windows).collect()
    }

    fn tooltip_for(&self, target: &HoverTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_tooltip(&self.niri, *window, clock)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                Some(self.window_group.inspect_tooltip(clock))
            }
            HoverTarget::Workspace(workspace) => workspace_tooltip::tooltip(
                &self.snapshot,
                &self.desktop_entries,
                NiriLayout::default(),
                *workspace,
            ),
            _ => None,
        }
    }
}

impl ShowcasedGroup {
    pub fn position(&self) -> Position {
        match self {
            ShowcasedGroup::BottomBar(_)
            | ShowcasedGroup::ClaudeUsage(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::AppearancePresets(_)
            | ShowcasedGroup::LoginSession(_)
            | ShowcasedGroup::Wallpaper(_) => Position::Bottom,
            ShowcasedGroup::WorkspaceSidebar(_) => Position::Left,
            ShowcasedGroup::Compute(_)
            | ShowcasedGroup::MemoryBoard(_)
            | ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::VoiceDictation(_)
            | ShowcasedGroup::Caldav(_)
            | ShowcasedGroup::Vpn(_)
            | ShowcasedGroup::Toasts(_) => Position::Top,
        }
    }

    pub fn group_contents<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<GroupContent<'a, ShowcaseMessage>> {
        match self {
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
            ShowcasedGroup::IdleTimeouts(group) => {
                vec![
                    group
                        .group_content(config)
                        .map(ShowcaseMessage::IdleTimeouts),
                ]
            }
            ShowcasedGroup::Wallpaper(group) => {
                vec![group.group_content().map(ShowcaseMessage::Wallpaper)]
            }
            ShowcasedGroup::AppearancePresets(menu) => {
                vec![
                    menu.group_content(config)
                        .map(ShowcaseMessage::AppearancePresets),
                ]
            }
            ShowcasedGroup::Audio(group) => {
                vec![group.group_content(config).map(ShowcaseMessage::Audio)]
            }
            ShowcasedGroup::Gamepad(gamepad) => gamepad
                .group
                .group_contents(&gamepad.bindings, None)
                .into_iter()
                .map(|group| group.map(ShowcaseMessage::Gamepad))
                .collect(),
            ShowcasedGroup::VoiceDictation(group) => {
                vec![group.group_content().map(ShowcaseMessage::VoiceDictation)]
            }
            ShowcasedGroup::Caldav(group) => {
                vec![group.group_content().map(ShowcaseMessage::Caldav)]
            }
            ShowcasedGroup::Vpn(group) => vec![group.group_content().map(ShowcaseMessage::Vpn)],
            ShowcasedGroup::LoginSession(group) => {
                vec![group.group_content().map(ShowcaseMessage::LoginSession)]
            }
            ShowcasedGroup::BottomBar(_)
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
            ShowcasedGroup::BottomBar(session) => session.bottom_bar_groups(config, clock),
            ShowcasedGroup::WorkspaceSidebar(session) => session.sidebar_groups(config, clock),
            _ => self
                .group_contents(config, clock)
                .into_iter()
                .filter_map(|group| group.view(config, position, clock))
                .collect(),
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
                    let _ = group.update(message);
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
                    let _ = gamepad.group.update(message, &gamepad.bindings, None);
                }
            }
            ShowcasedGroup::VoiceDictation(group) => {
                if let ShowcaseMessage::VoiceDictation(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Caldav(group) => {
                if let ShowcaseMessage::Caldav(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Vpn(group) => {
                if let ShowcaseMessage::Vpn(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::LoginSession(group) => {
                if let ShowcaseMessage::LoginSession(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                if let ShowcaseMessage::WindowGroup(message) = message {
                    let _ = session.window_group.update(message, clock);
                }
            }
            ShowcasedGroup::AppearancePresets(_) | ShowcasedGroup::Toasts(_) => {}
        }
    }

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
                memory_board::on_hover(*target)
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
            ShowcasedGroup::Vpn(group) => {
                let _ = group.hover(Some(target), config, None, clock);
                let HoverTarget::Vpn(target) = target else {
                    return Vec::new();
                };
                vpn::on_hover(target)
                    .into_iter()
                    .map(ShowcaseMessage::Vpn)
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
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                let _ = session
                    .window_group
                    .hover(&session.niri, Some(target), config, clock);
                niri_session::on_hover(target)
                    .into_iter()
                    .map(ShowcaseMessage::WindowGroup)
                    .collect()
            }
            ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Gamepad(_)
            | ShowcasedGroup::Caldav(_)
            | ShowcasedGroup::AppearancePresets(_)
            | ShowcasedGroup::Toasts(_) => Vec::new(),
        }
    }

    pub fn tooltip_for(
        &self,
        target: &HoverTarget,
        config: &Config,
        clock: Clock,
    ) -> Option<Tooltip> {
        match self {
            ShowcasedGroup::Compute(group) => {
                let HoverTarget::Compute(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::MemoryBoard(group) => {
                let HoverTarget::MemoryBoard(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::Weather(group) => {
                let HoverTarget::Weather(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, config, clock)
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                let HoverTarget::ClaudeUsage(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::Media(media) => {
                let HoverTarget::Media(target) = target else {
                    return None;
                };
                media
                    .group
                    .tooltip_for(target, &media.desktop_entries, clock)
            }
            ShowcasedGroup::IdleTimeouts(group) => {
                let HoverTarget::IdleTimeout(kind) = target else {
                    return None;
                };
                Some(group.tooltip_for(*kind, clock))
            }
            ShowcasedGroup::Wallpaper(group) => {
                let HoverTarget::Wallpaper(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::Audio(group) => {
                let HoverTarget::Audio(kind) = target else {
                    return None;
                };
                group.tooltip_for(*kind, clock)
            }
            ShowcasedGroup::Gamepad(gamepad) => {
                let HoverTarget::Gamepad(target) = target else {
                    return None;
                };
                gamepad
                    .group
                    .tooltip_for(*target, &gamepad.bindings, None, clock)
            }
            ShowcasedGroup::VoiceDictation(group) => {
                let HoverTarget::VoiceDictation(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::Caldav(group) => {
                let HoverTarget::Caldav(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::Vpn(group) => {
                let HoverTarget::Vpn(target) = target else {
                    return None;
                };
                group.tooltip_for(target, clock)
            }
            ShowcasedGroup::LoginSession(group) => {
                let HoverTarget::LoginSession(target) = target else {
                    return None;
                };
                group.tooltip_for(*target, clock)
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                session.tooltip_for(target, clock)
            }
            ShowcasedGroup::AppearancePresets(menu) => {
                let HoverTarget::AppearancePresets(target) = target else {
                    return None;
                };
                Some(menu.tooltip_for(*target))
            }
            ShowcasedGroup::Toasts(_) => None,
        }
    }

    pub fn interactive_panel<'a>(
        &self,
        target: &HoverTarget,
        config: &'a Config,
        appearance: &ShellAppearance,
    ) -> Option<Element<'a, ShowcaseMessage>> {
        match (self, target) {
            (
                ShowcasedGroup::AppearancePresets(menu),
                HoverTarget::AppearancePresets(AppearancePresetsTarget::Menu),
            ) => Some(
                menu.menu_panel(
                    config,
                    Radius::from(appearance.overlays.corner_radius_px),
                    appearance,
                )
                .map(ShowcaseMessage::AppearancePresets),
            ),
            _ => None,
        }
    }

    pub fn niri(&self) -> Option<&EventStreamState> {
        match self {
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                Some(&session.niri)
            }
            ShowcasedGroup::Toasts(toasts) => Some(&toasts.niri),
            ShowcasedGroup::Compute(_)
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
            | ShowcasedGroup::Vpn(_)
            | ShowcasedGroup::LoginSession(_)
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
