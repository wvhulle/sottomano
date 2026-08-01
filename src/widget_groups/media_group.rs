//! odin's `Media`: album-art handle that unfolds the track title, transport
//! controls, and a source-switch row for the MPRIS players on the bus, updated
//! signal-driven from [`mpris::subscription`]. Hidden when no player is
//! present. When two or more players have a loaded track, an app icon per
//! player sits after the transport buttons: clicking one re-points what the bar
//! controls (selection only — no playback change), clicking the active one
//! raises its window. Each icon is the player's real themed desktop icon
//! (resolved off-thread through the same pipeline the window list uses),
//! falling back to a Font Awesome glyph when none resolves. Deviation: odin's
//! circular art clipping is not ported.

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, image, mouse_area, svg},
};
use niri_ipc::state::EventStreamState;

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        Tooltip, TooltipBody,
        desktop_entries::DesktopIndex,
        font_awesome::{FontAwesome, Solid},
        mpris,
        mpris::{Media, Status},
        niri, row,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        menu_group::{MenuGroup, MenuMessage},
        text::bar_text,
    },
};

const MAX_TITLE: usize = 40;

pub struct MediaGroup {
    players: Vec<Media>,
    /// The bus the user pinned as the source to show and control; cleared when
    /// that player disappears. `None` falls back to the status-priority pick.
    forced: Option<String>,
    /// Playback status shown right after a transport press, until the next
    /// signal confirms it — so the button reacts instantly.
    optimistic: Option<Status>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(Vec<Media>),
    Menu(MenuMessage),
    Transport(&'static str),
    /// A player's icon in the source row was clicked (its bus name).
    Source(String),
    Tooltip(TooltipMessage),
}

impl MediaGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            players: Vec::new(),
            forced: None,
            optimistic: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<MediaMessage> {
        mpris::subscription().map(MediaMessage::Reading)
    }

    /// The player the bar currently shows and controls: the pinned source if it
    /// is still present, else the first playing, else the first paused, else
    /// the first available (odin's `_pickPlayer`).
    fn current(&self) -> Option<&Media> {
        self.forced
            .as_ref()
            .and_then(|bus| self.players.iter().find(|m| &m.bus == bus))
            .or_else(|| self.players.iter().find(|m| m.status == Status::Playing))
            .or_else(|| self.players.iter().find(|m| m.status == Status::Paused))
            .or_else(|| self.players.first())
    }

    /// The status to show: the optimistic override while a transport press is
    /// in flight, else the last reading.
    fn status(&self, media: &Media) -> Status {
        self.optimistic.unwrap_or(media.status)
    }

    pub fn update(&mut self, msg: MediaMessage, niri: &EventStreamState) -> Task<MediaMessage> {
        match msg {
            // A fresh signal reading is authoritative; drop the optimistic override.
            MediaMessage::Reading(reading) => {
                self.players = reading;
                self.optimistic = None;
                // Unpin a source that has gone away so the pick falls back.
                if let Some(bus) = &self.forced
                    && !self.players.iter().any(|m| &m.bus == bus)
                {
                    self.forced = None;
                }
            }
            MediaMessage::Menu(m) => {
                self.menu.update(m);
            }
            MediaMessage::Source(bus) => {
                // Re-clicking the active source raises its window; clicking any
                // other source re-points the bar to it without touching
                // playback.
                if self.current().is_some_and(|m| m.bus == bus) {
                    return self.raise(&bus, niri);
                }
                self.forced = Some(bus);
            }
            MediaMessage::Transport(method) => {
                let Some(media) = self.current() else {
                    return Task::none();
                };
                let bus = media.bus.clone();
                // Flip the shown status now so the button reacts instantly; the
                // next poll confirms (or corrects) it. No artificial delay.
                self.optimistic = match method {
                    "PlayPause" => Some(if self.status(media) == Status::Playing {
                        Status::Paused
                    } else {
                        Status::Playing
                    }),
                    "Stop" => Some(Status::Stopped),
                    _ => self.optimistic,
                };
                return blocking("mpris control", move || mpris::control(&bus, method)).discard();
            }
            MediaMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    /// Raise the app window of the player on `bus`, matched through niri IPC
    /// (MPRIS `Raise` is a no-op on Wayland for Spotify and others). App-handle
    /// preference mirrors odin's `raisePlayer`: desktop entry, then the bus
    /// handle, then the identity. Windowless daemons (mpd) match nothing and
    /// are left alone.
    fn raise(&self, bus: &str, niri: &EventStreamState) -> Task<MediaMessage> {
        let Some(media) = self.players.iter().find(|m| m.bus == bus) else {
            return Task::none();
        };
        let app_ids: Vec<String> = [
            media.desktop_entry.clone(),
            Some(mpris::app_handle(&media.bus)),
            Some(media.identity.to_lowercase()),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
        let title = (!media.title.is_empty()).then(|| media.title.clone());
        let Some(id) = niri::Placement::derive(niri).window_by_source(&app_ids, title.as_deref())
        else {
            return Task::none();
        };
        blocking("mpris raise", move || niri::focus_window(id)).discard()
    }

    pub fn icon_keys(&self) -> Vec<String> {
        self.players.iter().map(Media::app_id).collect()
    }

    /// The source-switch row: one app icon per player with a loaded track, the
    /// active source full-strength and the rest dimmed to the same `0.4` the
    /// bar uses elsewhere for "not the active thing". Omitted unless at least
    /// two players are switchable, since there is nothing to switch between.
    fn sources<'a>(
        &self,
        config: &ResolvedConfig,
        index: &DesktopIndex,
        current: &Media,
        reveal: f32,
    ) -> Option<Element<'a, MediaMessage>> {
        let usable: Vec<&Media> = self
            .players
            .iter()
            .filter(|m| !m.title.is_empty())
            .collect();
        if usable.len() < 2 {
            return None;
        }
        let side = config.font.icon_size_px;
        let icons: Vec<Element<'a, MediaMessage>> = usable
            .into_iter()
            .map(|m| {
                let alpha = reveal * if m.bus == current.bus { 1.0 } else { 0.4 };
                let icon = source_icon(config, index, m, side, alpha);
                mouse_area(icon)
                    .on_press(MediaMessage::Source(m.bus.clone()))
                    .into()
            })
            .collect();
        Some(
            Row::with_children(icons)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        matches!(target, HoverTarget::Media)
            .then(|| self.tooltip())
            .flatten()
    }

    fn tooltip(&self) -> Option<Tooltip> {
        let media = self.current()?;
        let mut rows = Vec::new();
        if !media.title.is_empty() {
            rows.push(row("Track", media.title.clone()));
        }
        if !media.artist.is_empty() {
            rows.push(row("Artist", media.artist.clone()));
        }
        if !media.album.is_empty() {
            rows.push(row("Album", media.album.clone()));
        }
        rows.push(row("Player", media.identity.clone()));
        rows.push(row("State", media.status.label()));
        Some(Tooltip {
            body: TooltipBody::Table(rows),
            footer: None,
        })
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        index: &DesktopIndex,
        position: Position,
    ) -> Option<Element<'a, MediaMessage>> {
        let media = self.current()?;
        let status = self.status(media);
        let alpha = if status == Status::Playing { 1.0 } else { 0.4 };

        let handle = handle(config, media, status, alpha);
        let handle = mouse_area(hoverable(handle, HoverTarget::Media, MediaMessage::Tooltip))
            .on_press(MediaMessage::Menu(MenuMessage::Toggle))
            .into();

        let reveal = self.menu.reveal();
        let title = bar_text(&config.font, truncate(&media.label()))
            .color(config.palette.bar_foreground.scale_alpha(reveal));
        let title = mouse_area(title).on_press(MediaMessage::Source(media.bus.clone()));
        let content = Row::with_children(
            [
                Some(title.into()),
                Some(transport(config, media, status, reveal)),
                self.sources(config, index, media, reveal),
            ]
            .into_iter()
            .flatten(),
        )
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center);

        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            MediaMessage::Menu,
        ))
    }
}

/// The persistent handle: the track's album art, or a fallback glyph.
fn handle<'a>(
    config: &ResolvedConfig,
    media: &Media,
    status: Status,
    alpha: f32,
) -> Element<'a, MediaMessage> {
    let side = config.font.icon_size_px;
    if let Some(art) = &media.art {
        return image(art.clone())
            .width(side)
            .height(side)
            .opacity(alpha)
            .into();
    }
    let icon = if status == Status::Playing {
        Solid::Music
    } else {
        Solid::Play
    };
    icon.widget()
        .size(side)
        .color(config.palette.bar_foreground.scale_alpha(alpha))
        .into()
}

fn source_icon<'a>(
    config: &ResolvedConfig,
    index: &DesktopIndex,
    media: &Media,
    side: f32,
    alpha: f32,
) -> Element<'a, MediaMessage> {
    if let Some(path) = index.app_icon(&media.app_id()) {
        let is_svg = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        return if is_svg {
            svg(svg::Handle::from_path(path))
                .width(side)
                .height(side)
                .opacity(alpha)
                .into()
        } else {
            image(image::Handle::from_path(path))
                .width(side)
                .height(side)
                .opacity(alpha)
                .into()
        };
    }
    player_icon(media)
        .widget()
        .size(side)
        .color(config.palette.bar_foreground.scale_alpha(alpha))
        .into()
}

/// The player's app glyph: the first Font Awesome slug that matches its desktop
/// entry, bus handle, or an identity word (odin's `MediaSource` glyph
/// resolution) — mpd's "Music Player Daemon" yields the solid `music` glyph.
/// Falls back to a generic music note when nothing matches. Used only when no
/// themed app icon resolved.
fn player_icon(media: &Media) -> FontAwesome {
    let mut handles = Vec::new();
    if let Some(desktop) = &media.desktop_entry
        && !desktop.is_empty()
    {
        handles.push(desktop.to_lowercase());
    }
    handles.push(mpris::app_handle(&media.bus).to_lowercase());
    handles.extend(
        media
            .identity
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string),
    );
    handles
        .iter()
        .find_map(|handle| FontAwesome::from_slug(handle))
        .unwrap_or(Solid::Music.into())
}

/// The transport row: previous / play-pause / stop / next, each shown only when
/// the player supports it.
fn transport<'a>(
    config: &ResolvedConfig,
    media: &Media,
    status: Status,
    reveal: f32,
) -> Element<'a, MediaMessage> {
    let color = config.palette.bar_foreground.scale_alpha(reveal * 0.9);
    let button = |icon: FontAwesome, method: &'static str| {
        mouse_area(icon.widget().size(config.font.icon_size_px).color(color))
            .on_press(MediaMessage::Transport(method))
            .into()
    };
    let toggle_icon = if status == Status::Playing {
        Solid::Pause.into()
    } else {
        Solid::Play.into()
    };
    let buttons: Vec<Element<'a, MediaMessage>> = [
        media
            .can_prev
            .then(|| button(Solid::BackwardStep.into(), "Previous")),
        media.can_toggle.then(|| button(toggle_icon, "PlayPause")),
        (status != Status::Stopped).then(|| button(Solid::Stop.into(), "Stop")),
        media
            .can_next
            .then(|| button(Solid::ForwardStep.into(), "Next")),
    ]
    .into_iter()
    .flatten()
    .collect();
    Row::with_children(buttons)
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

fn truncate(label: &str) -> String {
    if label.chars().count() > MAX_TITLE {
        let head: String = label.chars().take(MAX_TITLE - 1).collect();
        format!("{head}…")
    } else {
        label.to_string()
    }
}
