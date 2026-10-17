use iced::{Element, widget::container};
use niri_ipc::state::EventStreamState;

use crate::{
    config::Config,
    data::source::{
        desktop_entries::DesktopIndex, niri::Snapshot, niri_config::ConfiguredWorkspaces,
    },
    desktop_shell::{
        Message, hover_tooltips::HoverTarget, monitor_surfaces::Position, pointer_drag::DragView,
    },
    readme_showcase::fixtures::{compute, niri_session},
    widget_groups::{
        ai_group::AiGroup,
        audio_group::AudioGroup,
        compute_group::ComputeGroup,
        idle_timeouts_group::IdleTimeoutsGroup,
        media_group::MediaGroup,
        wallpaper_group::WallpaperGroup,
        weather_group::WeatherGroup,
        window_group::{WindowGroup, WindowTarget},
        workspaces_group::{self, workspace_tooltip},
    },
    widgets::{
        continuous_reading::ContinuousReading, favicon::Favicons, text::FontFallbacks,
        tooltip_body::Tooltip, window_icon::WindowAssets,
    },
};

pub const SHOWCASE_OUTPUT: &str = "readme-showcase";

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
    Weather(Box<WeatherGroup>),
    Ai(Box<AiGroup>),
    Media(Box<ShowcasedMedia>),
    IdleTimeouts(Box<IdleTimeoutsGroup>),
    Wallpaper(Box<WallpaperGroup>),
    Audio(Box<AudioGroup>),
    BottomBar(Box<ShowcasedNiriSession>),
    WorkspaceSidebar(Box<ShowcasedNiriSession>),
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
    pub font_fallbacks: FontFallbacks,
}

impl ShowcasedNiriSession {
    fn sidebar_groups<'a>(&'a self, config: &'a Config) -> Vec<Element<'a, Message>> {
        self.snapshot
            .get()
            .and_then(|snapshot| {
                workspaces_group::vertical_view(
                    snapshot,
                    SHOWCASE_OUTPUT,
                    config,
                    &undragged(),
                    Some(&configured_in_fixture_order(snapshot)),
                )
            })
            .map(|workspaces| workspaces.map(Message::from))
            .into_iter()
            .collect()
    }

    fn bottom_bar_groups<'a>(&'a self, config: &'a Config) -> Vec<Element<'a, Message>> {
        let Some(snapshot) = self.snapshot.get() else {
            return Vec::new();
        };
        let workspaces = workspaces_group::inline_view(
            snapshot,
            SHOWCASE_OUTPUT,
            config,
            Position::Bottom,
            &undragged(),
            Some(&configured_in_fixture_order(snapshot)),
        )
        .map(|workspaces| workspaces.map(Message::from));
        let windows = self
            .window_group
            .view(
                snapshot,
                WindowAssets {
                    entries: &self.desktop_entries,
                    favicons: &self.favicons,
                    fallbacks: &self.font_fallbacks,
                },
                None,
                SHOWCASE_OUTPUT,
                config,
                Position::Bottom,
                &undragged(),
            )
            .map(|element| element.map(|event| Message::from(event.map(Message::WindowGroup))));
        workspaces.into_iter().chain(windows).collect()
    }

    fn tooltip_for(&self, target: &HoverTarget, config: &Config) -> Option<Tooltip> {
        match target {
            HoverTarget::Window(WindowTarget::Pick(window)) => {
                WindowGroup::pick_tooltip(&self.niri, &self.snapshot, *window)
            }
            HoverTarget::Window(WindowTarget::Inspect(_)) => {
                Some(self.window_group.inspect_tooltip())
            }
            HoverTarget::Workspace(workspace) => workspace_tooltip::tooltip(
                &self.snapshot,
                &self.desktop_entries,
                &config.theme.font,
                &self.font_fallbacks,
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
            | ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Ai(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_) => Position::Top,
        }
    }

    pub fn panel_groups<'a>(&'a self, config: &'a Config) -> Vec<Element<'a, Message>> {
        let position = self.position();
        let single = match self {
            ShowcasedGroup::Compute(group) => group
                .group_content()
                .map(Message::Compute)
                .view(config, position),
            ShowcasedGroup::Weather(group) => group
                .group_content()
                .map(Message::Weather)
                .view(config, position),
            ShowcasedGroup::Ai(group) => group
                .group_content()
                .map(Message::Ai)
                .view(config, position),
            ShowcasedGroup::Media(media) => media
                .group
                .group_content(config, &media.desktop_entries)
                .map(Message::Media)
                .view(config, position),
            ShowcasedGroup::IdleTimeouts(group) => group
                .group_content(config)
                .map(Message::IdleTimeouts)
                .view(config, position),
            ShowcasedGroup::Wallpaper(group) => group
                .group_content()
                .map(Message::Wallpaper)
                .view(config, position),
            ShowcasedGroup::Audio(group) => group
                .group_content(config)
                .map(Message::Audio)
                .view(config, position),
            ShowcasedGroup::BottomBar(session) => return session.bottom_bar_groups(config),
            ShowcasedGroup::WorkspaceSidebar(session) => return session.sidebar_groups(config),
        };
        single.into_iter().collect()
    }

    pub fn update(&mut self, message: Message) {
        match self {
            ShowcasedGroup::Compute(group) => {
                if let Message::Compute(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::Weather(group) => {
                if let Message::Weather(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::Ai(group) => {
                if let Message::Ai(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::Media(media) => {
                if let Message::Media(message) = message {
                    let _ = media.group.update(message);
                }
            }
            ShowcasedGroup::IdleTimeouts(group) => {
                if let Message::IdleTimeouts(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::Wallpaper(group) => {
                if let Message::Wallpaper(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::Audio(group) => {
                if let Message::Audio(message) = message {
                    let _ = group.update(message);
                }
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                if let Message::WindowGroup(message) = message {
                    let _ = session.window_group.update(message);
                }
            }
        }
    }

    pub fn hover(&mut self, target: &HoverTarget, config: &Config) {
        match self {
            ShowcasedGroup::Compute(group) => {
                let _ = group.hover(Some(target), config);
                if let HoverTarget::Compute(target) = target {
                    compute::on_hover(*target).into_iter().for_each(|reading| {
                        let _ = group.update(reading);
                    });
                }
            }
            ShowcasedGroup::Ai(group) => {
                let _ = group.hover(Some(target), config);
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                let _ = session
                    .window_group
                    .hover(&session.niri, Some(target), config);
                niri_session::on_hover(target)
                    .into_iter()
                    .for_each(|reading| {
                        let _ = session.window_group.update(reading);
                    });
            }
            ShowcasedGroup::Weather(_)
            | ShowcasedGroup::Media(_)
            | ShowcasedGroup::IdleTimeouts(_)
            | ShowcasedGroup::Wallpaper(_)
            | ShowcasedGroup::Audio(_) => {}
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget, config: &Config) -> Option<Tooltip> {
        match self {
            ShowcasedGroup::Compute(group) => {
                let HoverTarget::Compute(target) = target else {
                    return None;
                };
                group.tooltip_for(*target)
            }
            ShowcasedGroup::Weather(group) => {
                let HoverTarget::Weather(target) = target else {
                    return None;
                };
                group.tooltip_for(*target)
            }
            ShowcasedGroup::Ai(group) => {
                let HoverTarget::Ai(target) = target else {
                    return None;
                };
                group.tooltip_for(*target)
            }
            ShowcasedGroup::Media(media) => {
                let HoverTarget::Media(target) = target else {
                    return None;
                };
                media.group.tooltip_for(target)
            }
            ShowcasedGroup::IdleTimeouts(group) => {
                let HoverTarget::IdleTimeout(kind) = target else {
                    return None;
                };
                Some(group.tooltip_for(*kind))
            }
            ShowcasedGroup::Wallpaper(group) => {
                let HoverTarget::Wallpaper(target) = target else {
                    return None;
                };
                group.tooltip_for(*target)
            }
            ShowcasedGroup::Audio(group) => {
                let HoverTarget::Audio(kind) = target else {
                    return None;
                };
                group.tooltip_for(*kind)
            }
            ShowcasedGroup::BottomBar(session) | ShowcasedGroup::WorkspaceSidebar(session) => {
                session.tooltip_for(target, config)
            }
        }
    }
}
