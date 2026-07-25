//! odin's `Media`: album-art handle that unfolds the track title and transport
//! controls for the current MPRIS player. Hidden when no player is present.
//! Deviations: polled (not signal-driven), and odin's source-switch row and
//! circular art clipping are not ported.

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, image, mouse_area},
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        Tooltip, TooltipBody, mpris,
        mpris::{Media, Status},
        row,
    },
    layer_surface::Position,
    message_subscriptions::blocking,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::{
        icon::Icon,
        menu_group::{MenuGroup, MenuMessage},
        text::bar_text,
    },
};

const MAX_TITLE: usize = 40;

pub struct MediaGroup {
    cell: SourceCell<Media>,
    /// Playback status shown right after a transport press, until the next poll
    /// confirms it — so the button reacts instantly with no artificial delay.
    optimistic: Option<Status>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(Option<Media>),
    Menu(MenuMessage),
    Transport(&'static str),
    Tooltip(TooltipMessage),
}

impl MediaGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            cell: SourceCell::default(),
            optimistic: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<MediaMessage> {
        SourceCell::subscription().map(MediaMessage::Reading)
    }

    /// The status to show: the optimistic override while a transport press is
    /// in flight, else the last poll.
    fn status(&self, media: &Media) -> Status {
        self.optimistic.unwrap_or(media.status)
    }

    pub fn update(&mut self, msg: MediaMessage) -> Task<MediaMessage> {
        match msg {
            // A fresh poll is authoritative; drop the optimistic override.
            MediaMessage::Reading(reading) => {
                self.cell.load(reading);
                self.optimistic = None;
            }
            MediaMessage::Menu(m) => {
                self.menu.update(m);
            }
            MediaMessage::Transport(method) => {
                let Some(media) = self.cell.get() else {
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

    pub fn tooltip(&self) -> Option<Tooltip> {
        let media = self.cell.get()?;
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
        position: Position,
    ) -> Option<Element<'a, MediaMessage>> {
        let media = self.cell.get()?;
        let status = self.status(media);
        let alpha = if status == Status::Playing { 1.0 } else { 0.4 };

        let handle = handle(config, media, status, alpha);
        let handle = mouse_area(hoverable(handle, TooltipSource::Media, MediaMessage::Tooltip))
            .on_press(MediaMessage::Menu(MenuMessage::Toggle))
            .into();

        let reveal = self.menu.reveal();
        let title = bar_text(&config.font, truncate(&media.label()))
            .color(config.palette.bar_foreground.scale_alpha(reveal));
        let content = Row::with_children(
            [Some(title.into()), Some(transport(config, media, status, reveal))]
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

/// The persistent handle: album art (a `file://` art url) or a fallback glyph.
fn handle<'a>(
    config: &ResolvedConfig,
    media: &Media,
    status: Status,
    alpha: f32,
) -> Element<'a, MediaMessage> {
    let side = config.font.icon_size_px;
    if let Some(path) = media.art_url.as_deref().and_then(|u| u.strip_prefix("file://")) {
        return image(image::Handle::from_path(path))
            .width(side)
            .height(side)
            .opacity(alpha)
            .into();
    }
    let icon = if status == Status::Playing {
        Icon::Music
    } else {
        Icon::Play
    };
    icon.solid()
        .size(side)
        .color(config.palette.bar_foreground.scale_alpha(alpha))
        .into()
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
    let button = |icon: Icon, method: &'static str| {
        mouse_area(icon.solid().size(config.font.icon_size_px).color(color))
            .on_press(MediaMessage::Transport(method))
            .into()
    };
    let toggle_icon = if status == Status::Playing {
        Icon::Pause
    } else {
        Icon::Play
    };
    let buttons: Vec<Element<'a, MediaMessage>> = [
        media.can_prev.then(|| button(Icon::BackwardStep, "Previous")),
        media.can_toggle.then(|| button(toggle_icon, "PlayPause")),
        (status != Status::Stopped).then(|| button(Icon::Stop, "Stop")),
        media.can_next.then(|| button(Icon::ForwardStep, "Next")),
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
