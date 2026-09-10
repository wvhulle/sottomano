use iced::{
    Alignment, Element, Font, Subscription, Task,
    font::Style as FontStyle,
    widget::{Row, button, image, rich_text, svg, text::Span},
};
use niri_ipc::state::EventStreamState;

use crate::{
    config::Config,
    data::source::{
        desktop_entries::DesktopIndex,
        mpris,
        mpris::{Media, Status},
        niri,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        flat_button,
        font_awesome::{FontAwesome, Solid},
        menu_group::{MenuGroup, MenuMessage},
        text::{LINE_HEIGHT, colored_span},
        tooltip_body::{Tooltip, row},
    },
};

const SEPARATOR: &str = " · ";

#[derive(Default)]
pub struct MediaGroup {
    players: Vec<Media>,
    forced: Option<String>,
    optimistic: Option<Status>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(Vec<Media>),
    Menu(MenuMessage),
    Transport(&'static str),
    Source(String),
    Tooltip(TooltipMessage),
}

impl MediaGroup {
    pub fn subscription() -> Subscription<MediaMessage> {
        mpris::subscription().map(MediaMessage::Reading)
    }

    fn current(&self) -> Option<&Media> {
        self.forced
            .as_ref()
            .and_then(|bus| self.players.iter().find(|m| &m.bus == bus))
            .or_else(|| self.players.iter().find(|m| m.status == Status::Playing))
            .or_else(|| self.players.iter().find(|m| m.status == Status::Paused))
            .or_else(|| self.players.first())
    }

    fn status(&self, media: &Media) -> Status {
        self.optimistic.unwrap_or(media.status)
    }

    pub fn update(&mut self, msg: MediaMessage, niri: &EventStreamState) -> Task<MediaMessage> {
        match msg {
            MediaMessage::Reading(reading) => {
                self.players = reading;
                self.optimistic = None;
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
        blocking("mpris raise", move || {
            niri::send_action(&niri_ipc::Action::FocusWindow { id });
        })
        .discard()
    }

    pub fn icon_keys(&self) -> Vec<String> {
        self.players.iter().map(Media::app_id).collect()
    }

    fn sources<'a>(
        &self,
        config: &Config,
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
                let opacity = reveal * if m.bus == current.bus { 1.0 } else { 0.4 };
                let icon = source_icon(config, index, m, side, opacity);
                button(icon)
                    .padding(0)
                    .style(flat_button(config.palette.bar_foreground))
                    .on_press(MediaMessage::Source(m.bus.clone()))
                    .into()
            })
            .collect();
        Some(
            Row::with_children(icons)
                .spacing(config.spacing.widget_spacing_px)
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
        Some(Tooltip::table(rows))
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        index: &'a DesktopIndex,
        position: Position,
    ) -> Option<Element<'a, MediaMessage>> {
        let media = self.current()?;
        let status = self.status(media);
        let opacity = if status == Status::Playing { 1.0 } else { 0.4 };

        let handle = move || {
            let face = handle(config, media, status, opacity);
            button(hoverable(face, HoverTarget::Media, MediaMessage::Tooltip))
                .padding(0)
                .style(flat_button(config.palette.bar_foreground))
                .on_press(MediaMessage::Menu(MenuMessage::Toggle))
                .into()
        };

        let content = move |reveal: f32| {
            let title = rich_text(label_spans(config, media, reveal))
                .size(config.font.text_size_px)
                .line_height(LINE_HEIGHT);
            let title = button(title)
                .padding(0)
                .style(flat_button(config.palette.bar_foreground))
                .on_press(MediaMessage::Source(media.bus.clone()));
            Row::with_children(
                [
                    Some(title.into()),
                    Some(transport(config, media, status, reveal)),
                    self.sources(config, index, media, reveal),
                ]
                .into_iter()
                .flatten(),
            )
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into()
        };

        Some(
            self.menu
                .view(config, position, handle, content, MediaMessage::Menu),
        )
    }
}

fn handle<'a>(
    config: &Config,
    media: &Media,
    status: Status,
    opacity: f32,
) -> Element<'a, MediaMessage> {
    let side = config.font.icon_size_px;
    if let Some(art) = &media.art {
        return image(art.clone())
            .width(side)
            .height(side)
            .opacity(opacity)
            .into();
    }
    let icon = if status == Status::Playing {
        Solid::Music
    } else {
        Solid::Play
    };
    icon.glyph()
        .size(side)
        .color(config.palette.bar_foreground.scale_alpha(opacity))
        .into()
}

fn source_icon<'a>(
    config: &Config,
    index: &DesktopIndex,
    media: &Media,
    side: f32,
    opacity: f32,
) -> Element<'a, MediaMessage> {
    if let Some(path) = index.app_icon(&media.app_id()) {
        let is_svg = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        return if is_svg {
            svg(svg::Handle::from_path(path))
                .width(side)
                .height(side)
                .opacity(opacity)
                .into()
        } else {
            image(image::Handle::from_path(path))
                .width(side)
                .height(side)
                .opacity(opacity)
                .into()
        };
    }
    player_icon(media)
        .glyph()
        .size(side)
        .color(config.palette.bar_foreground.scale_alpha(opacity))
        .into()
}

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

fn transport<'a>(
    config: &Config,
    media: &Media,
    status: Status,
    reveal: f32,
) -> Element<'a, MediaMessage> {
    let color = config.palette.bar_foreground.scale_alpha(reveal * 0.9);
    let button = |icon: FontAwesome, method: &'static str| {
        button(icon.glyph().size(config.font.icon_size_px).color(color))
            .padding(0)
            .style(flat_button(color))
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
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
}

fn label_spans<'a>(config: &Config, media: &Media, reveal: f32) -> Vec<Span<'a>> {
    let base = config.font.iced_font;
    let italic = Font {
        style: FontStyle::Italic,
        ..base
    };
    let lead = config.palette.inactive.scale_alpha(reveal);
    let main = config.palette.bar_foreground.scale_alpha(reveal);
    match (media.artist.as_str(), media.title.as_str()) {
        ("", "") => vec![colored_span(media.identity.clone(), base, main)],
        ("", title) => vec![colored_span(title.to_owned(), base, main)],
        (artist, "") => vec![colored_span(artist.to_owned(), italic, main)],
        (artist, title) => vec![
            colored_span(artist.to_owned(), italic, lead),
            colored_span(SEPARATOR.to_owned(), base, lead),
            colored_span(title.to_owned(), base, main),
        ],
    }
}
