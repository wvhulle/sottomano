use std::time::{Duration, Instant};

use iced::{
    Alignment, Element, Font, Subscription, Task,
    font::Style as FontStyle,
    time::every,
    widget::{Row, button, image, rich_text, svg, text::Span},
};
use niri_ipc::state::EventStreamState;

use crate::{
    config::Config,
    data::source::{
        desktop_entries::DesktopIndex,
        mpris,
        mpris::{Media, Progress, Status},
        niri,
    },
    desktop_shell::hover_tooltips::{HoverTarget, hoverable},
    message_subscriptions::blocking,
    widgets::{
        bar_widget::Glyph,
        flat_button,
        gauge::Gauge,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        text::{LINE_HEIGHT, colored_span},
        tooltip_body::{Tooltip, row},
        widget_group::{ExpansionMessage, GroupEvent, GroupExpansion, WidgetGroup},
    },
};

const SEPARATOR: &str = " · ";
const PROGRESS_TICK: Duration = Duration::from_secs(1);

pub struct MediaGroup {
    players: Vec<Media>,
    forced: Option<String>,
    optimistic: Option<Status>,
    now: Instant,
    pub(crate) expansion: GroupExpansion,
}

impl Default for MediaGroup {
    fn default() -> Self {
        Self {
            players: Vec::new(),
            forced: None,
            optimistic: None,
            now: Instant::now(),
            expansion: GroupExpansion::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(Vec<Media>),
    Tick(Instant),
    Expansion(ExpansionMessage),
    Transport(&'static str),
    Source(String),
}

impl MediaGroup {
    pub fn subscription(&self) -> Subscription<MediaMessage> {
        let readings = mpris::subscription().map(MediaMessage::Reading);
        if self.is_progressing() {
            Subscription::batch([readings, every(PROGRESS_TICK).map(MediaMessage::Tick)])
        } else {
            readings
        }
    }

    fn is_progressing(&self) -> bool {
        self.current().is_some_and(|media| {
            self.status(media) == Status::Playing
                && matches!(media.progress, Progress::Timed { .. })
        })
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
            MediaMessage::Tick(now) => self.now = now,
            MediaMessage::Reading(reading) => {
                self.players = reading;
                self.now = Instant::now();
                self.optimistic = None;
                if let Some(bus) = &self.forced
                    && !self.players.iter().any(|m| &m.bus == bus)
                {
                    self.forced = None;
                }
            }
            MediaMessage::Expansion(m) => {
                self.expansion.update(m);
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
        let Some(id) =
            niri::Placement::derive(niri).window_by_source(&app_ids, media.title.as_deref())
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
        let usable: Vec<&Media> = self.players.iter().filter(|m| m.title.is_some()).collect();
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
        let mut rows: Vec<_> = [
            ("Track", &media.title),
            ("Artist", &media.artist),
            ("Album", &media.album),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(row(key, value.clone()?)))
        .collect();
        rows.push(row("Player", media.identity.clone()));
        rows.push(row("State", media.status.label()));
        if let Some(remaining) = media.progress.remaining(self.status(media), self.now) {
            rows.push(row(
                "Remaining",
                humantime::format_duration(Duration::from_secs(remaining.as_secs())).to_string(),
            ));
        }
        Some(Tooltip::table(rows))
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        index: &'a DesktopIndex,
    ) -> WidgetGroup<'a, MediaMessage> {
        let group = WidgetGroup::new(&self.expansion, MediaMessage::Expansion);
        let Some(media) = self.current() else {
            return group;
        };
        let status = self.status(media);
        let opacity = if status == Status::Playing { 1.0 } else { 0.4 };
        let sources = self.sources(config, index, media, 1.0).is_some().then(|| {
            GroupItem::element(move |reveal| {
                self.sources(config, index, media, reveal)
                    .unwrap_or_else(|| Row::new().into())
            })
        });
        group
            .item(GroupItem::hoverable_element(move |_| {
                hoverable(
                    track_face(config, media, status, opacity, self.now).map(GroupEvent::Group),
                    HoverTarget::Media,
                    GroupEvent::Tooltip,
                )
            }))
            .item(
                GroupItem::element(move |reveal| {
                    rich_text(label_spans(config, media, reveal))
                        .size(config.font.text_size_px)
                        .line_height(LINE_HEIGHT)
                        .into()
                })
                .on_press(MediaMessage::Source(media.bus.clone())),
            )
            .item(GroupItem::element(move |reveal| {
                transport(config, media, status, reveal)
            }))
            .item(sources)
    }
}

fn track_face<'a>(
    config: &Config,
    media: &Media,
    status: Status,
    opacity: f32,
    now: Instant,
) -> Element<'a, MediaMessage> {
    let side = config.font.icon_size_px;
    let cover = track_cover(config, media, status, opacity, side);
    let Some(fraction) = media.progress.fraction(status, now) else {
        return cover;
    };
    let color = config.palette.bar_foreground.scale_alpha(opacity);
    Row::with_children([cover, Gauge::new(fraction, color, side).into()])
        .spacing(icon_value_gap(config))
        .align_y(Alignment::Center)
        .into()
}

fn track_cover<'a>(
    config: &Config,
    media: &Media,
    status: Status,
    opacity: f32,
    side: f32,
) -> Element<'a, MediaMessage> {
    if let Some(art) = &media.art {
        return image(art.clone())
            .width(side)
            .height(side)
            .opacity(opacity)
            .into();
    }
    let icon = if status == Status::Playing {
        Phosphor::MusicNote(Weight::Regular)
    } else {
        Phosphor::Play(Weight::Regular)
    };
    Glyph::from(icon.with_weight(Weight::Regular))
        .view(side, config.palette.bar_foreground.scale_alpha(opacity))
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
    Glyph::from(player_icon(media).with_weight(Weight::Regular))
        .view(side, config.palette.bar_foreground.scale_alpha(opacity))
}

fn player_icon(media: &Media) -> Phosphor {
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
        .find_map(|handle| player_brand(handle))
        .unwrap_or(Phosphor::MusicNote(Weight::Regular))
}

fn player_brand(handle: &str) -> Option<Phosphor> {
    match handle {
        "spotify" => Some(Phosphor::SpotifyLogo(Weight::Regular)),
        "tidal" | "tidal-hifi" => Some(Phosphor::TidalLogo(Weight::Regular)),
        "soundcloud" => Some(Phosphor::SoundcloudLogo(Weight::Regular)),
        "youtube" | "youtube-music" => Some(Phosphor::YoutubeLogo(Weight::Regular)),
        "twitch" => Some(Phosphor::TwitchLogo(Weight::Regular)),
        "chromium" | "chrome" | "google-chrome" => {
            Some(Phosphor::GoogleChromeLogo(Weight::Regular))
        }
        "discord" => Some(Phosphor::DiscordLogo(Weight::Regular)),
        "steam" => Some(Phosphor::SteamLogo(Weight::Regular)),
        "podcasts" | "apple-podcasts" => Some(Phosphor::ApplePodcastsLogo(Weight::Regular)),
        _ => None,
    }
}

fn transport<'a>(
    config: &Config,
    media: &Media,
    status: Status,
    reveal: f32,
) -> Element<'a, MediaMessage> {
    let color = config.palette.bar_foreground.scale_alpha(reveal * 0.9);
    let button = |icon: Phosphor, method: &'static str| {
        button(Glyph::from(icon.with_weight(Weight::Regular)).view(config.font.icon_size_px, color))
            .padding(0)
            .style(flat_button(color))
            .on_press(MediaMessage::Transport(method))
            .into()
    };
    let toggle_icon = if status == Status::Playing {
        Phosphor::Pause(Weight::Regular)
    } else {
        Phosphor::Play(Weight::Regular)
    };
    let buttons: Vec<Element<'a, MediaMessage>> = [
        media
            .can_prev
            .then(|| button(Phosphor::SkipBack(Weight::Regular), "Previous")),
        media.can_toggle.then(|| button(toggle_icon, "PlayPause")),
        (status != Status::Stopped).then(|| button(Phosphor::Stop(Weight::Regular), "Stop")),
        media
            .can_next
            .then(|| button(Phosphor::SkipForward(Weight::Regular), "Next")),
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
    let Some(artist) = &media.artist else {
        let title = media.title.as_ref().unwrap_or(&media.identity);
        return vec![colored_span(title.clone(), base, main)];
    };
    let Some(title) = &media.title else {
        return vec![colored_span(artist.clone(), italic, main)];
    };
    vec![
        colored_span(artist.clone(), italic, lead),
        colored_span(SEPARATOR.to_owned(), base, lead),
        colored_span(title.clone(), base, main),
    ]
}
