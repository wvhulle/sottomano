use std::time::{Duration, Instant};

use iced::{
    Alignment, Element, Subscription, Task,
    time::every,
    widget::{Row, button, image, rich_text, svg, text::Span},
};
use uom::si::f64::Ratio;

use crate::{
    config::Config,
    data::source::{
        bus::OwnerProcess,
        desktop_entries::DesktopIndex,
        mpris,
        mpris::{Command, Media, Progress, Status},
        niri::WindowOwner,
    },
    desktop_shell::{
        app_window_focus::AppWindowFocus,
        hover_tooltips::{HoverTarget, PressHints, hoverable},
    },
    message_subscriptions::blocking,
    widgets::{
        bar_widget::Glyph,
        confirm_button::{ConfirmButton, Press},
        flat_button,
        gauge::Gauge,
        group_item::{GroupItem, Press as GroupPress},
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        text::{LINE_HEIGHT, colored_span},
        tooltip_body::{Tooltip, TooltipBody},
        widget_group::{
            ExpansionMessage, GroupEvent, GroupExpansion, GroupHandle, HandleFace, WidgetGroup,
        },
    },
};

pub mod seek_bar;
pub mod session_controls;
pub mod track_tooltip;
pub mod transport_controls;

use self::{
    seek_bar::{SeekBar, SeekMessage},
    session_controls::SessionControls,
    track_tooltip::{playback_section, search_section, track_section},
    transport_controls::TransportControls,
};

const PROGRESS_TICK: Duration = Duration::from_secs(1);
const RAISE_PLAYER_HINT: &str = "bring the player's window forward";

pub struct MediaGroup {
    players: Vec<Media>,
    forced: Option<String>,
    seek_bar: SeekBar,
    quit_button: ConfirmButton,
    now: Instant,
    pub(crate) expansion: GroupExpansion,
}

impl Default for MediaGroup {
    fn default() -> Self {
        Self {
            players: Vec::new(),
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
    Reading(Vec<Media>),
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
            media.status == Status::Playing && matches!(media.progress, Progress::Timed { .. })
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

    pub fn update(&mut self, msg: MediaMessage) -> Action {
        match msg {
            MediaMessage::Tick(now) => self.now = now,
            MediaMessage::Reading(reading) => {
                self.players = reading;
                self.now = Instant::now();
                if let Some(bus) = &self.forced
                    && !self.players.iter().any(|m| &m.bus == bus)
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

    pub fn icon_keys(&self) -> Vec<String> {
        self.players
            .iter()
            .filter_map(|media| media.desktop_entry.declared())
            .map(str::to_owned)
            .collect()
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
        Some(Tooltip::of(TooltipBody::sections(&[
            track_section(media),
            playback_section(media, self.now),
            search_section(media),
        ])))
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
        index: &'a DesktopIndex,
    ) -> WidgetGroup<'a, MediaMessage> {
        let Some(media) = self.current() else {
            return WidgetGroup::new(
                &self.expansion,
                MediaMessage::Expansion,
                GroupHandle {
                    face: HandleFace::FirstItem,
                    preview: None,
                    forwards_scroll: false,
                },
                &[Option::<GroupItem<'a, MediaMessage>>::None; 0],
            );
        };
        let opacity = if media.status == Status::Playing {
            1.0
        } else {
            0.4
        };
        let sources = self.sources(config, index, media, 1.0).is_some().then(|| {
            GroupItem::element(move |reveal| {
                self.sources(config, index, media, reveal)
                    .unwrap_or_else(|| Row::new().into())
                    .map(GroupEvent::Group)
            })
        });
        let seekable = self.seek_bar.view(config, media, self.now).is_some();
        let seek = seekable.then(|| {
            GroupItem::element(move |_| {
                self.seek_bar
                    .view(config, media, self.now)
                    .map_or_else(|| Row::new().into(), |bar| bar.map(MediaMessage::Seek))
                    .map(GroupEvent::Group)
            })
        });
        let gauge_fraction = media
            .progress
            .fraction(media.status, self.now)
            .filter(|_| !(seekable && self.expansion.is_expanded()));
        let session = (!self.session_controls(config, media, 1.0).is_empty()).then(|| {
            GroupItem::element(move |reveal| {
                Element::<MediaMessage>::from(self.session_controls(config, media, reveal))
                    .map(GroupEvent::Group)
            })
        });
        WidgetGroup::new(
            &self.expansion,
            MediaMessage::Expansion,
            GroupHandle {
                face: HandleFace::FirstItem,
                preview: None,
                forwards_scroll: seekable,
            },
            &[
                Some(GroupItem::element(move |_| {
                    hoverable(
                        track_face(config, media, opacity, gauge_fraction).map(GroupEvent::Group),
                        HoverTarget::Media,
                        PressHints::default(),
                        GroupEvent::Tooltip,
                    )
                })),
                Some(track_label(config, media)),
                seek,
                Some(GroupItem::element(move |reveal| {
                    Element::<MediaMessage>::from(TransportControls::new(
                        config,
                        media,
                        reveal,
                        MediaMessage::Command,
                    ))
                    .map(GroupEvent::Group)
                })),
                session,
                sources,
            ],
        )
    }

    fn session_controls<'a>(
        &'a self,
        config: &'a Config,
        media: &'a Media,
        reveal: f32,
    ) -> SessionControls<'a, MediaMessage> {
        SessionControls::new(
            config,
            media,
            &self.quit_button,
            reveal,
            MediaMessage::Command,
            MediaMessage::Quit,
            MediaMessage::CancelQuit,
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
                .size(config.font.text_size_px)
                .line_height(LINE_HEIGHT)
                .into(),
            HoverTarget::Media,
            press_hints,
            GroupEvent::Tooltip,
        )
    });
    if raisable {
        GroupItem {
            on_press: Some(GroupPress {
                does: RAISE_PLAYER_HINT,
                message: MediaMessage::Command(Command::Raise),
            }),
            ..label
        }
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
    let side = config.font.icon_size_px;
    let cover = track_cover(config, media, opacity, side);
    let Some(fraction) = gauge_fraction else {
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
    Glyph::from(Phosphor::MusicNote(Weight::Regular))
        .view(side, config.palette.bar_foreground.scale_alpha(opacity))
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
        .declared()
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
    Glyph::from(Phosphor::MusicNote(Weight::Regular))
        .view(side, config.palette.bar_foreground.scale_alpha(opacity))
}

fn label_spans<'a>(config: &Config, media: &Media, reveal: f32) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![colored_span(
        title.clone(),
        config.font.iced_font,
        config.palette.bar_foreground.scale_alpha(reveal),
    )]
}
