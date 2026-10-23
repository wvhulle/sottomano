use iced::{
    Element, Length, Padding, border,
    widget::{Column, container, stack, text},
};
use niri_ipc::state::EventStreamState;
use sottomano_config::{Config, screen_edges::Position};
use sottomano_sources::{
    desktop_notifications::notification_data::NotificationData,
    desktop_windows_apps::{niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot::Snapshot},
};
use sottomano_state::niri_rules::NiriLayout;
use sottomano_widgets::{
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::HoverTarget,
    shell_message::Message,
    widget_groups::{
        audio_group::AudioGroup,
        claude_usage_group::ClaudeUsageGroup,
        compute_group::ComputeGroup,
        idle_timeouts_group::IdleTimeoutsGroup,
        media_group::MediaGroup,
        memory_board_group::MemoryBoardGroup,
        wallpaper_group::WallpaperGroup,
        weather_group::WeatherGroup,
        window_group::{WindowGroup, WindowTarget},
        workspaces_group::{inline_strip, sidebar_column, workspace_tooltip},
    },
    widgets::{
        continuous_reading::ContinuousReading, favicon::Favicons, palette_colors::PaletteColors,
        pointer_drag::DragView, tooltip_body::Tooltip, window_icon::WindowAssets,
    },
};

use crate::fixtures::{compute, memory_board, niri_session};

pub const SHOWCASE_OUTPUT: &str = "widget-showcase";

fn configured_in_fixture_order(snapshot: &Snapshot) -> ConfiguredWorkspaces {
    snapshot
        .workspaces_for_output(SHOWCASE_OUTPUT)
        .iter()
        .filter_map(|p| p.name.clone())
        .collect()
}

fn undragged() -> DragView {
    DragView {
        drag: None,
        ghost_frame: container::Style::default(),
    }
}

pub enum ShowcasedGroup {
    Compute(Box<ComputeGroup>),
    MemoryBoard(Box<MemoryBoardGroup>),
    Weather(Box<WeatherGroup>),
    ClaudeUsage(Box<ClaudeUsageGroup>),
    Media(Box<ShowcasedMedia>),
    IdleTimeouts(Box<IdleTimeoutsGroup>),
    Wallpaper(Box<WallpaperGroup>),
    Audio(Box<AudioGroup>),
    BottomBar(Box<ShowcasedNiriSession>),
    WorkspaceSidebar(Box<ShowcasedNiriSession>),
    Toasts(Box<ShowcasedToasts>),
}

pub struct ShowcasedToasts {
    pub niri: EventStreamState,
    pub opening: Vec<NotificationData>,
}

impl ShowcasedToasts {
    fn window_frames<'a>(&'a self, config: &'a Config) -> Element<'a, Message> {
        let palette = PaletteColors::from(&config.theme.palette);
        let frames = self.niri.windows.windows.values().filter_map(|window| {
            let (x, y) = window.layout.tile_pos_in_workspace_view?;
            let (width, height) = window.layout.window_size;
            let title = window
                .title
                .as_deref()
                .map(|title| text(title).color(palette.bar_foreground).into());
            let frame = container(Column::with_children(title))
                .padding(config.layout.overlay.content_inset)
                .width(Length::Fixed(f32::from(u16::try_from(width).ok()?)))
                .height(Length::Fixed(f32::from(u16::try_from(height).ok()?)))
                .style(move |_| container::Style {
                    background: Some(palette.surface.into()),
                    border: border::rounded(config.layout.overlay.content_inset)
                        .color(palette.bar_foreground.scale_alpha(0.4))
                        .width(1.0),
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
    pub snapshot: ContinuousReading<Snapshot>,
    pub desktop_entries: DesktopIndex,
    pub favicons: Favicons,
}

impl ShowcasedNiriSession {
    fn sidebar_groups<'a>(&'a self, config: &'a Config, clock: Clock) -> Vec<Element<'a, Message>> {
        self.snapshot
            .get()
            .and_then(|snapshot| {
                sidebar_column::vertical_view(
                    snapshot,
                    SHOWCASE_OUTPUT,
                    config,
                    &undragged(),
                    Some(&configured_in_fixture_order(snapshot)),
                    clock,
                )
            })
            .map(|workspaces| workspaces.map(Message::from))
            .into_iter()
            .collect()
    }

    fn bottom_bar_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, Message>> {
        let Some(snapshot) = self.snapshot.get() else {
            return Vec::new();
        };
        let workspaces = inline_strip::inline_view(
            snapshot,
            SHOWCASE_OUTPUT,
            config,
            Position::Bottom,
            &undragged(),
            Some(&configured_in_fixture_order(snapshot)),
            clock,
        )
        .map(|workspaces| workspaces.map(Message::from));
        let windows = self
            .window_group
            .view(
                snapshot,
                WindowAssets {
                    entries: &self.desktop_entries,
                    favicons: &self.favicons,
                },
                None,
                SHOWCASE_OUTPUT,
                config,
                Position::Bottom,
                &undragged(),
                clock,
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))));
        workspaces.into_iter().chain(windows).collect()
    }

    fn tooltip_for(&self, target: &HoverTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_tooltip(&self.niri, &self.snapshot, *window, clock)
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
            ShowcasedGroup::BottomBar(_) => Position::Bottom,
            ShowcasedGroup::WorkspaceSidebar(_) => Position::Left,
            ShowcasedGroup::Compute(_)
            | ShowcasedGroup::MemoryBoard(_)
            | ShowcasedGroup::Weather(_)
            | ShowcasedGroup::ClaudeUsage(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Toasts(_) => Position::Top,
        }
    }

    pub fn panel_groups<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> Vec<Element<'a, Message>> {
        let position = self.position();
        let single = match self {
            ShowcasedGroup::Compute(group) => group
                .group_content()
                .map(Message::Compute)
                .view(config, position, clock),
            ShowcasedGroup::MemoryBoard(group) => group
                .group_content()
                .map(Message::MemoryBoard)
                .view(config, position, clock),
            ShowcasedGroup::Weather(group) => group
                .group_content(config)
                .map(Message::Weather)
                .view(config, position, clock),
            ShowcasedGroup::ClaudeUsage(group) => group
                .group_content()
                .map(Message::ClaudeUsage)
                .view(config, position, clock),
            ShowcasedGroup::Media(media) => media
                .group
                .group_content(config, &media.desktop_entries, clock)
                .map(Message::Media)
                .view(config, position, clock),
            ShowcasedGroup::IdleTimeouts(group) => group
                .group_content(config)
                .map(Message::IdleTimeouts)
                .view(config, position, clock),
            ShowcasedGroup::Wallpaper(group) => group
                .group_content()
                .map(Message::Wallpaper)
                .view(config, position, clock),
            ShowcasedGroup::Audio(group) => group
                .group_content(config)
                .map(Message::Audio)
                .view(config, position, clock),
            ShowcasedGroup::BottomBar(session) => return session.bottom_bar_groups(config, clock),
            ShowcasedGroup::WorkspaceSidebar(session) => {
                return session.sidebar_groups(config, clock);
            }
            ShowcasedGroup::Toasts(_) => return Vec::new(),
        };
        single.into_iter().collect()
    }

    pub fn update(&mut self, message: Message, clock: Clock) {
        match self {
            ShowcasedGroup::Compute(group) => {
                if let Message::Compute(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::MemoryBoard(group) => {
                if let Message::MemoryBoard(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Weather(group) => {
                if let Message::Weather(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                if let Message::ClaudeUsage(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Media(media) => {
                if let Message::Media(message) = message {
                    let _ = media.group.update(message, clock);
                }
            }
            ShowcasedGroup::IdleTimeouts(group) => {
                if let Message::IdleTimeouts(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::Wallpaper(group) => {
                if let Message::Wallpaper(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::Audio(group) => {
                if let Message::Audio(message) = message {
                    let _ = group.update(message, clock);
                }
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                if let Message::WindowGroup(message) = message {
                    let _ = session.window_group.update(message, clock);
                }
            }
            ShowcasedGroup::Toasts(_) => {}
        }
    }

    pub fn hover(&mut self, target: &HoverTarget, config: &Config, clock: Clock) {
        match self {
            ShowcasedGroup::Compute(group) => {
                let _ = group.hover(Some(target), config, clock);
                if let HoverTarget::Compute(target) = target {
                    compute::on_hover(*target).into_iter().for_each(|reading| {
                        let _ = group.update(reading, clock);
                    });
                }
            }
            ShowcasedGroup::MemoryBoard(group) => {
                let _ = group.hover(Some(target), config, clock);
                if let HoverTarget::MemoryBoard(target) = target {
                    memory_board::on_hover(*target)
                        .into_iter()
                        .for_each(|reading| {
                            let _ = group.update(reading, clock);
                        });
                }
            }
            ShowcasedGroup::ClaudeUsage(group) => {
                let _ = group.hover(Some(target), config, clock);
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                let _ = session
                    .window_group
                    .hover(&session.niri, Some(target), config, clock);
                niri_session::on_hover(target)
                    .into_iter()
                    .for_each(|reading| {
                        let _ = session.window_group.update(reading, clock);
                    });
            }
            ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_)
            | ShowcasedGroup::Toasts(_) => {}
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
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                session.tooltip_for(target, clock)
            }
            ShowcasedGroup::Toasts(_) => None,
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
            | ShowcasedGroup::Audio(_) => None,
        }
    }

    pub fn opening_toasts(&self) -> &[NotificationData] {
        match self {
            ShowcasedGroup::Toasts(toasts) => &toasts.opening,
            _ => &[],
        }
    }

    pub fn backdrop<'a>(&'a self, config: &'a Config) -> Option<Element<'a, Message>> {
        match self {
            ShowcasedGroup::Toasts(toasts) => Some(toasts.window_frames(config)),
            _ => None,
        }
    }
}
