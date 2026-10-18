use std::time::{Duration, Instant};

use iced::{
    Alignment, Element, Subscription, Task,
    time::every,
    widget::{Row, button, image, rich_text, svg, text::Span},
};
use uom::si::f64::Ratio;
use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    background_work::{blocking, source_subscription},
    bus::{DesktopBuses, OwnerProcess},
    desktop_entries::DesktopIndex,
    mpris::{self, Command, Media, Progress, Status},
    niri::WindowOwner,
};

use crate::{
    hover_tooltips::{HoverTarget, PressHints, hoverable},
    shell_message::AppWindowFocus,
    widgets::{
        bar_widget::Glyph,
        confirm_button::{ConfirmButton, Press},
        continuous_reading::ContinuousReading,
        flat_button,
        gauge::Gauge,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        text::{LINE_HEIGHT, colored_span},
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion},
    },
};

mod seek_bar;
mod session_controls;
mod track_tooltip;
mod transport_controls;

use self::{
    seek_bar::{SeekBar, SeekMessage},
    session_controls::{SessionControls, SessionMessages},
    track_tooltip::{playback_section, search_section, track_section},
    transport_controls::TransportControls,
};

const PROGRESS_TICK: Duration = Duration::from_secs(1);
const RAISE_PLAYER_HINT: &str = "bring the player's window forward";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaTarget {
    Track,
    Source(String),
    Seek,
    Shuffle,
    Previous,
    PlayPause,
    Next,
    Loop,
    Stop,
    Quit,
}

impl From<MediaTarget> for HoverTarget {
    fn from(target: MediaTarget) -> Self {
        HoverTarget::Media(target)
    }
}

pub struct MediaGroup {
    players: ContinuousReading<Vec<Media>>,
    forced: Option<String>,
    seek_bar: SeekBar,
    quit_button: ConfirmButton,
    now: Instant,
    expansion: GroupExpansion,
}

impl Default for MediaGroup {
    fn default() -> Self {
        Self {
            players: ContinuousReading::default(),
            forced: None,
            seek_bar: SeekBar::default(),
            quit_button: ConfirmButton::new(true),
            now: Instant::now(),
            expansion: GroupExpansion::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(SourceDelivery<Vec<Media>>),
    Tick(Instant),
    Expansion(ExpansionMessage),
    Command(Command),
    Seek(SeekMessage),
    Quit,
    CancelQuit,
    Source(String),
    RaiseByMpris,
}

pub enum Action {
    Run(Task<MediaMessage>),
    FocusAppWindow(AppWindowFocus<MediaMessage>),
}

impl MediaGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<MediaMessage> {
        let readings = source_subscription::<Vec<Media>>(buses.clone()).map(MediaMessage::Reading);
        if self.is_progressing() {
            Subscription::batch([readings, every(PROGRESS_TICK).map(MediaMessage::Tick)])
        } else {
            readings
        }
    }

    fn is_progressing(&self) -> bool {
        self.current().is_some_and(|media| {
            media.status == Status::Playing && matches!(media.progress, Progress::Timed { .. })
        })
    }

    fn current(&self) -> Option<&Media> {
        let players = self.players.get()?;
        self.forced
            .as_ref()
            .and_then(|bus| players.iter().find(|m| &m.bus == bus))
            .or_else(|| players.iter().find(|m| m.status == Status::Playing))
            .or_else(|| players.iter().find(|m| m.status == Status::Paused))
            .or_else(|| players.first())
    }

    pub fn update(&mut self, msg: MediaMessage) -> Action {
        match msg {
            MediaMessage::Tick(now) => self.now = now,
            MediaMessage::Reading(delivery) => {
                self.players.receive_keeping_last_reading(delivery);
                self.now = Instant::now();
                if let Some(bus) = &self.forced
                    && !self
                        .players
                        .get()
                        .into_iter()
                        .flatten()
                        .any(|m| &m.bus == bus)
                {
                    self.forced = None;
                }
                if let Some(media) = self.current().cloned() {
                    self.seek_bar.player_reported(&media, self.now);
                }
            }
            MediaMessage::Expansion(ExpansionMessage::HandleScrolled(delta)) => {
                return Action::Run(self.seek_by(SeekMessage::Scroll(delta)));
            }
            MediaMessage::Expansion(m) => {
                self.expansion.update(m);
            }
            MediaMessage::Source(bus) => return self.select(bus),
            MediaMessage::Command(Command::Raise) => return self.raise(),
            MediaMessage::RaiseByMpris => return Action::Run(self.raise_by_mpris()),
            MediaMessage::Command(command) => return Action::Run(self.send(command)),
            MediaMessage::Seek(message) => return Action::Run(self.seek_by(message)),
            MediaMessage::CancelQuit => self.quit_button.disarm(),
            MediaMessage::Quit => match self.quit_button.press() {
                Press::Armed => {}
                Press::Fire => return Action::Run(self.send(Command::Quit)),
            },
        }
        Action::Run(Task::none())
    }

    fn seek_by(&mut self, message: SeekMessage) -> Task<MediaMessage> {
        let Some(media) = self.current().cloned() else {
            return Task::none();
        };
        let Some(command) = self.seek_bar.update(message, &media, self.now) else {
            return Task::none();
        };
        self.seek(command)
    }

    fn select(&mut self, bus: String) -> Action {
        let Some(current) = self.current() else {
            return Action::Run(Task::none());
        };
        if current.bus == bus {
            return self.raise();
        }
        self.forced = Some(bus);
        self.seek_bar.reset();
        self.quit_button.disarm();
        Action::Run(Task::none())
    }

    fn raise(&self) -> Action {
        let Some(OwnerProcess::Identified(pid)) = self.current().map(|media| media.owner) else {
            return Action::Run(self.raise_by_mpris());
        };
        Action::FocusAppWindow(AppWindowFocus {
            owner: WindowOwner::Process(pid),
            unmatched: MediaMessage::RaiseByMpris,
        })
    }

    fn raise_by_mpris(&self) -> Task<MediaMessage> {
        if self.current().is_some_and(|media| media.capabilities.raise) {
            self.send(Command::Raise)
        } else {
            Task::none()
        }
    }

    fn send(&self, command: Command) -> Task<MediaMessage> {
        self.dispatch(command).discard()
    }

    fn seek(&self, command: Command) -> Task<MediaMessage> {
        self.dispatch(command).then(|accepted| {
            if accepted {
                Task::none()
            } else {
                Task::done(MediaMessage::Seek(SeekMessage::Rejected))
            }
        })
    }

    fn dispatch(&self, command: Command) -> Task<bool> {
        let Some(media) = self.current() else {
            return Task::none();
        };
        let bus = media.bus.clone();
        blocking("mpris control", move || mpris::control(&bus, command))
    }

    #[must_use]
    pub fn icon_keys(&self) -> Vec<String> {
        self.players
            .get()
            .into_iter()
            .flatten()
            .filter_map(|media| media.desktop_entry.clone())
            .collect()
    }

    fn titled_players(&self) -> Vec<&Media> {
        self.players
            .get()
            .into_iter()
            .flatten()
            .filter(|m| m.title.is_some())
            .collect()
    }

    fn sources<'a>(
        config: &Config,
        index: &DesktopIndex,
        players: &[&Media],
        current: &Media,
        reveal: f32,
    ) -> Element<'a, GroupEvent<MediaMessage>> {
        let side = config.theme.font.icon_size_px;
        let icons: Vec<Element<'a, GroupEvent<MediaMessage>>> = players
            .iter()
            .map(|m| {
                let selected = m.bus == current.bus;
                let opacity = reveal * if selected { 1.0 } else { 0.4 };
                let icon = source_icon(config, index, m, side, opacity).map(GroupEvent::Group);
                hoverable(
                    button(icon)
                        .padding(0)
                        .style(flat_button(config.theme.palette.bar_foreground))
                        .on_press(GroupEvent::Group(MediaMessage::Source(m.bus.clone())))
                        .into(),
                    MediaTarget::Source(m.bus.clone()).into(),
                    PressHints {
                        left: (!selected).then_some("control this player"),
                        ..PressHints::default()
                    },
                    GroupEvent::Tooltip,
                )
            })
            .collect();
        Row::with_children(icons)
            .spacing(config.layout.panel.widget_spacing)
            .align_y(Alignment::Center)
            .into()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &MediaTarget, index: &DesktopIndex) -> Option<Tooltip> {
        let media = self.current()?;
        match target {
            MediaTarget::Track => self.players.tooltip_with(TooltipBody::sections([
                track_section(media, index),
                playback_section(media, self.now),
                search_section(media, index),
            ])),
            MediaTarget::Source(bus) => {
                let player = self.titled_players().into_iter().find(|m| &m.bus == bus)?;
                self.players
                    .tooltip_with(TooltipBody::dictionary([KeyValue::new(
                        "Player",
                        player.identity.clone(),
                    )]))
            }
            MediaTarget::Seek
            | MediaTarget::Shuffle
            | MediaTarget::Previous
            | MediaTarget::PlayPause
            | MediaTarget::Next
            | MediaTarget::Loop
            | MediaTarget::Stop
            | MediaTarget::Quit => self.players.tooltip_with(TooltipBody::sections([])),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        index: &'a DesktopIndex,
    ) -> GroupContent<'a, MediaMessage> {
        let Some(media) = self.current() else {
            return GroupContent::new(&self.expansion, MediaMessage::Expansion, []);
        };
        let opacity = if media.status == Status::Playing {
            1.0
        } else {
            0.4
        };
        let players = self.titled_players();
        let sources = (players.len() >= 2).then(|| {
            GroupItem::element(move |reveal| Self::sources(config, index, &players, media, reveal))
        });
        let seekable = SeekBar::seekable(media, self.now);
        let seek = seekable.then(|| {
            GroupItem::element(move |_| {
                hoverable(
                    self.seek_bar
                        .view(config, media, self.now)
                        .map_or_else(|| Row::new().into(), |bar| bar.map(MediaMessage::Seek))
                        .map(GroupEvent::Group),
                    MediaTarget::Seek.into(),
                    PressHints {
                        left: Some("jump to this point"),
                        scroll: Some("seek"),
                        ..PressHints::default()
                    },
                    GroupEvent::Tooltip,
                )
            })
        });
        let gauge_fraction = media
            .progress
            .fraction(media.status, self.now)
            .filter(|_| !(seekable && self.expansion.is_expanded()));
        let session = (!self.session_controls(config, media, 1.0).is_empty()).then(|| {
            GroupItem::element(move |reveal| self.session_controls(config, media, reveal).into())
        });
        GroupContent::new(
            &self.expansion,
            MediaMessage::Expansion,
            [
                Some(GroupItem::hoverable_element(
                    HoverTarget::Media(MediaTarget::Track),
                    move |_| {
                        track_face(config, media, opacity, gauge_fraction).map(GroupEvent::Group)
                    },
                )),
                Some(track_label(config, media)),
                seek,
                Some(GroupItem::element(move |reveal| {
                    TransportControls::new(
                        config,
                        media,
                        reveal,
                        |command| GroupEvent::Group(MediaMessage::Command(command)),
                        GroupEvent::Tooltip,
                    )
                    .into()
                })),
                session,
                sources,
            ]
            .into_iter()
            .flatten(),
        )
        .forwards_scroll(seekable.then_some("seek in the track"))
    }

    fn session_controls<'a>(
        &'a self,
        config: &'a Config,
        media: &'a Media,
        reveal: f32,
    ) -> SessionControls<'a, GroupEvent<MediaMessage>> {
        SessionControls::new(
            config,
            media,
            &self.quit_button,
            reveal,
            SessionMessages {
                command: |command| GroupEvent::Group(MediaMessage::Command(command)),
                quit: GroupEvent::Group(MediaMessage::Quit),
                cancel_quit: GroupEvent::Group(MediaMessage::CancelQuit),
                tooltip: GroupEvent::Tooltip,
            },
        )
    }
}

fn track_label<'a>(config: &'a Config, media: &'a Media) -> GroupItem<'a, MediaMessage> {
    let raisable = media.owner != OwnerProcess::Unidentified || media.capabilities.raise;
    let press_hints = PressHints {
        left: raisable.then_some(RAISE_PLAYER_HINT),
        ..PressHints::default()
    };
    let label = GroupItem::element(move |reveal| {
        hoverable(
            rich_text(label_spans(config, media, reveal))
                .size(config.theme.font.text_size_px)
                .line_height(LINE_HEIGHT)
                .into(),
            HoverTarget::Media(MediaTarget::Track),
            press_hints,
            GroupEvent::Tooltip,
        )
    });
    if raisable {
        label.on_press(RAISE_PLAYER_HINT, MediaMessage::Command(Command::Raise))
    } else {
        label
    }
}

fn track_face<'a>(
    config: &Config,
    media: &Media,
    opacity: f32,
    gauge_fraction: Option<Ratio>,
) -> Element<'a, MediaMessage> {
    let side = config.theme.font.icon_size_px;
    let cover = track_cover(config, media, opacity, side);
    let Some(fraction) = gauge_fraction else {
        return cover;
    };
    let color = config.theme.palette.bar_foreground.scale_alpha(opacity);
    Row::with_children([cover, Gauge::new(fraction, color, side).into()])
        .spacing(icon_value_gap(config))
        .align_y(Alignment::Center)
        .into()
}

fn track_cover<'a>(
    config: &Config,
    media: &Media,
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
    Glyph::from(Phosphor::MusicNote(Weight::Regular)).view(
        side,
        config.theme.palette.bar_foreground.scale_alpha(opacity),
    )
}

fn source_icon<'a>(
    config: &Config,
    index: &DesktopIndex,
    media: &Media,
    side: f32,
    opacity: f32,
) -> Element<'a, MediaMessage> {
    let icon_path = media
        .desktop_entry
        .as_deref()
        .and_then(|entry| index.app_icon(entry));
    if let Some(path) = icon_path {
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
    Glyph::from(Phosphor::MusicNote(Weight::Regular)).view(
        side,
        config.theme.palette.bar_foreground.scale_alpha(opacity),
    )
}

fn label_spans<'a>(config: &Config, media: &Media, reveal: f32) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![colored_span(
        title.clone(),
        config.theme.font.iced_font,
        config.theme.palette.bar_foreground.scale_alpha(reveal),
    )]
}
