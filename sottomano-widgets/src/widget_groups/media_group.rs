use std::{
    borrow::Cow,
    collections::HashMap,
    time::{Duration, Instant},
};

use iced::{Element, Subscription, Task, time::every, widget::Row};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    blocking_background_work::blocking,
    desktop_windows_apps::niri_window_placement::WindowOwner,
    media_playback_audio::mpris_players::{self, Command, Media, Progress, Status},
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OwnerProcess},
};

use crate::{
    background_work::source_subscription,
    clock::Clock,
    desktop_index::DesktopIndex,
    hover_tooltips::{HoverTarget, PressHints, hoverable},
    shell_message::AppWindowFocus,
    widgets::{
        confirm_button::{ConfirmButton, Press},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        icon_handle::IconHandle,
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupEvent, GroupExpansion},
    },
};

mod seek_bar;
mod session_controls;
mod track_face;
mod track_tooltip;
mod transport_controls;

use self::{
    seek_bar::{SeekBar, SeekMessage},
    session_controls::{SessionControls, SessionMessages},
    track_face::{player_face, track_cover, track_label},
    track_tooltip::{playback_section, search_section, track_section},
    transport_controls::TransportControls,
};

const PROGRESS_TICK: Duration = Duration::from_secs(1);
const RAISE_PLAYER_HINT: &str = "bring the player's window forward";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaTarget {
    Player,
    Track,
    Seek,
    Shuffle,
    Previous,
    PlayPause,
    Next,
    Loop,
    Stop,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTarget {
    pub bus: Cow<'static, str>,
    pub control: MediaTarget,
}

impl PlayerTarget {
    #[must_use]
    pub fn of(bus: &str, control: MediaTarget) -> Self {
        Self {
            bus: Cow::Owned(bus.to_owned()),
            control,
        }
    }
}

impl From<PlayerTarget> for HoverTarget {
    fn from(target: PlayerTarget) -> Self {
        HoverTarget::Media(target)
    }
}

#[derive(Default)]
pub struct MediaGroup {
    players: ContinuousReading<Vec<Media>>,
    sessions: HashMap<String, PlayerSession>,
}

struct PlayerSession {
    cover_art: Option<CoverArt>,
    seek_bar: SeekBar,
    quit_button: ConfirmButton,
    expansion: GroupExpansion,
}

impl Default for PlayerSession {
    fn default() -> Self {
        Self {
            cover_art: None,
            seek_bar: SeekBar::default(),
            quit_button: ConfirmButton::new(true),
            expansion: GroupExpansion::default(),
        }
    }
}

struct CoverArt {
    url: String,
    handle: IconHandle,
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(SourceDelivery<Vec<Media>>),
    ArtLoaded {
        bus: String,
        url: String,
        handle: Option<IconHandle>,
    },
    ProgressAdvanced,
    Player {
        bus: String,
        message: PlayerMessage,
    },
}

#[derive(Debug, Clone)]
pub enum PlayerMessage {
    Expansion(ExpansionMessage),
    Command(Command),
    Seek(SeekMessage),
    Quit,
    CancelQuit,
    RaiseByMpris,
}

pub enum Action {
    Run(Task<MediaMessage>),
    FocusAppWindow(AppWindowFocus<MediaMessage>),
}

fn player_message(bus: &str, message: PlayerMessage) -> MediaMessage {
    MediaMessage::Player {
        bus: bus.to_owned(),
        message,
    }
}

fn lifted_to_player<'a>(
    bus: &'a str,
    element: Element<'a, GroupEvent<PlayerMessage>>,
) -> Element<'a, GroupEvent<MediaMessage>> {
    element.map(move |event| event.map(|message| player_message(bus, message)))
}

impl MediaGroup {
    pub fn collapse(&mut self) {
        for session in self.sessions.values_mut() {
            session.expansion.collapse();
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<MediaMessage> {
        let readings = source_subscription::<Vec<Media>>(buses.clone()).map(MediaMessage::Reading);
        if self.is_progressing() {
            Subscription::batch([
                readings,
                every(PROGRESS_TICK).map(|_| MediaMessage::ProgressAdvanced),
            ])
        } else {
            readings
        }
    }

    fn is_progressing(&self) -> bool {
        self.all_players().any(|media| {
            media.status == Status::Playing && matches!(media.progress, Progress::Timed { .. })
        })
    }

    fn all_players(&self) -> impl Iterator<Item = &Media> {
        self.players.get().into_iter().flatten()
    }

    fn player(&self, bus: &str) -> Option<&Media> {
        self.all_players().find(|media| media.bus == bus)
    }

    fn titled_players(&self) -> Vec<&Media> {
        let mut titled: Vec<&Media> = self
            .all_players()
            .filter(|media| media.title.is_some())
            .collect();
        titled.sort_by(|left, right| left.bus.cmp(&right.bus));
        titled
    }

    pub fn update(&mut self, msg: MediaMessage, clock: Clock) -> Action {
        match msg {
            MediaMessage::ProgressAdvanced => {}
            MediaMessage::Reading(delivery) => {
                self.players.receive_keeping_last_reading(delivery, clock);
                self.follow_players(clock.instant);
                return Action::Run(self.art_loads());
            }
            MediaMessage::ArtLoaded { bus, url, handle } => {
                let still_shown = self
                    .player(&bus)
                    .is_some_and(|media| media.art_url.as_deref() == Some(url.as_str()));
                if let Some(handle) = handle.filter(|_| still_shown)
                    && let Some(session) = self.sessions.get_mut(&bus)
                {
                    session.cover_art = Some(CoverArt { url, handle });
                }
            }
            MediaMessage::Player { bus, message } => {
                return self.update_player(&bus, message, clock);
            }
        }
        Action::Run(Task::none())
    }

    fn follow_players(&mut self, now: Instant) {
        let players: Vec<Media> = self.all_players().cloned().collect();
        self.sessions
            .retain(|bus, _| players.iter().any(|media| &media.bus == bus));
        for media in &players {
            let session = self.sessions.entry(media.bus.clone()).or_default();
            session.seek_bar.player_reported(media, now);
            if session
                .cover_art
                .as_ref()
                .is_some_and(|art| media.art_url.as_deref() != Some(art.url.as_str()))
            {
                session.cover_art = None;
            }
        }
    }

    fn update_player(&mut self, bus: &str, message: PlayerMessage, clock: Clock) -> Action {
        let (Some(media), Some(session)) = (self.player(bus).cloned(), self.sessions.get_mut(bus))
        else {
            return Action::Run(Task::none());
        };
        let task = match message {
            PlayerMessage::Expansion(ExpansionMessage::HandleScrolled(delta)) => {
                seek_by(session, &media, SeekMessage::Scroll(delta), clock.instant)
            }
            PlayerMessage::Expansion(expansion) => {
                session.expansion.update(expansion);
                Task::none()
            }
            PlayerMessage::Command(Command::Raise) => return raise(&media),
            PlayerMessage::RaiseByMpris => raise_by_mpris(&media),
            PlayerMessage::Command(command) => send(&media, command),
            PlayerMessage::Seek(seek) => seek_by(session, &media, seek, clock.instant),
            PlayerMessage::CancelQuit => {
                session.quit_button.disarm();
                Task::none()
            }
            PlayerMessage::Quit => match session.quit_button.press() {
                Press::Armed => Task::none(),
                Press::Fire => send(&media, Command::Quit),
            },
        };
        Action::Run(task)
    }

    fn art_loads(&self) -> Task<MediaMessage> {
        Task::batch(self.all_players().filter_map(|media| {
            let shown = self
                .sessions
                .get(&media.bus)
                .and_then(|session| session.cover_art.as_ref());
            let url = media.art_url.clone().filter(|_| shown.is_none())?;
            let bus = media.bus.clone();
            let loading = {
                let url = url.clone();
                async move { mpris_players::load_art(&url).await }
            };
            Some(Task::perform(loading, move |pixels| {
                MediaMessage::ArtLoaded {
                    bus: bus.clone(),
                    url: url.clone(),
                    handle: pixels.as_ref().map(IconHandle::from),
                }
            }))
        }))
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: &PlayerTarget,
        index: &DesktopIndex,
        clock: Clock,
    ) -> Option<Tooltip> {
        let media = self.player(&target.bus)?;
        match target.control {
            MediaTarget::Player => self.players.tooltip_with(TooltipBody::dictionary([
                KeyValue::new("Player", media.identity.clone()),
                KeyValue::new("State", media.status.label()),
            ])),
            MediaTarget::Track => self.players.tooltip_with(TooltipBody::sections([
                track_section(media, index),
                playback_section(media, clock.instant),
                search_section(media, index),
            ])),
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

    pub fn group_contents<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> impl Iterator<Item = GroupContent<'a, MediaMessage>> {
        self.titled_players().into_iter().filter_map(move |media| {
            let session = self.sessions.get(&media.bus)?;
            Some(player_group(config, clock, media, session))
        })
    }
}

fn player_group<'a>(
    config: &'a Config,
    clock: Clock,
    media: &'a Media,
    session: &'a PlayerSession,
) -> GroupContent<'a, MediaMessage> {
    let bus = media.bus.as_str();
    let opacity = if media.status == Status::Playing {
        1.0
    } else {
        0.4
    };
    let seekable = SeekBar::seekable(media, clock.instant);
    let gauge_fraction = media
        .progress
        .fraction(media.status, clock.instant)
        .filter(|_| !(seekable && session.expansion.is_expanded()));
    let face =
        GroupItem::hoverable_element(PlayerTarget::of(bus, MediaTarget::Player), move |_| {
            player_face(config, media, opacity, gauge_fraction)
        });
    let cover =
        GroupItem::hoverable_element(PlayerTarget::of(bus, MediaTarget::Track), move |_| {
            track_cover(
                config,
                session.cover_art.as_ref().map(|art| &art.handle),
                opacity,
            )
        });
    let seek = seekable.then(|| {
        GroupItem::element(move |_| {
            let bar = session
                .seek_bar
                .view(config, media, clock.instant)
                .map_or_else(
                    || Row::new().into(),
                    |bar| bar.map(|seek| GroupEvent::Group(PlayerMessage::Seek(seek))),
                );
            lifted_to_player(
                bus,
                hoverable(
                    bar,
                    PlayerTarget::of(bus, MediaTarget::Seek).into(),
                    PressHints {
                        left: Some("jump to this point"),
                        scroll: Some("seek"),
                        ..PressHints::default()
                    },
                    GroupEvent::Tooltip,
                ),
            )
        })
    });
    let transport = GroupItem::element(move |reveal| {
        lifted_to_player(
            bus,
            TransportControls::new(
                config,
                media,
                reveal,
                |command| GroupEvent::Group(PlayerMessage::Command(command)),
                GroupEvent::Tooltip,
            )
            .into(),
        )
    });
    let session_buttons = (!session_controls(config, media, session, 1.0).is_empty()).then(|| {
        GroupItem::element(move |reveal| {
            lifted_to_player(bus, session_controls(config, media, session, reveal).into())
        })
    });
    GroupContent::new(
        &session.expansion,
        move |expansion| player_message(bus, PlayerMessage::Expansion(expansion)),
        [
            Some(face),
            Some(cover),
            Some(track_label(config, media)),
            seek,
            Some(transport),
            session_buttons,
        ]
        .into_iter()
        .flatten(),
    )
    .forwards_scroll(seekable.then_some("seek in the track"))
}

fn session_controls<'a>(
    config: &'a Config,
    media: &'a Media,
    session: &'a PlayerSession,
    reveal: f32,
) -> SessionControls<'a, GroupEvent<PlayerMessage>> {
    SessionControls::new(
        config,
        media,
        &session.quit_button,
        reveal,
        SessionMessages {
            command: |command| GroupEvent::Group(PlayerMessage::Command(command)),
            quit: GroupEvent::Group(PlayerMessage::Quit),
            cancel_quit: GroupEvent::Group(PlayerMessage::CancelQuit),
            tooltip: GroupEvent::Tooltip,
        },
    )
}

fn seek_by(
    session: &mut PlayerSession,
    media: &Media,
    message: SeekMessage,
    now: Instant,
) -> Task<MediaMessage> {
    let Some(command) = session.seek_bar.update(message, media, now) else {
        return Task::none();
    };
    let bus = media.bus.clone();
    dispatch(media, command).then(move |accepted| {
        if accepted {
            Task::none()
        } else {
            Task::done(player_message(
                &bus,
                PlayerMessage::Seek(SeekMessage::Rejected),
            ))
        }
    })
}

fn raise(media: &Media) -> Action {
    let OwnerProcess::Identified(pid) = media.owner else {
        return Action::Run(raise_by_mpris(media));
    };
    Action::FocusAppWindow(AppWindowFocus {
        owner: WindowOwner::Process(pid),
        unmatched: player_message(&media.bus, PlayerMessage::RaiseByMpris),
    })
}

fn raise_by_mpris(media: &Media) -> Task<MediaMessage> {
    if media.capabilities.raise {
        send(media, Command::Raise)
    } else {
        Task::none()
    }
}

fn send(media: &Media, command: Command) -> Task<MediaMessage> {
    dispatch(media, command).discard()
}

fn dispatch(media: &Media, command: Command) -> Task<bool> {
    let bus = media.bus.clone();
    Task::future(blocking("mpris control", move || {
        mpris_players::control(&bus, command)
    }))
}

#[cfg(test)]
mod tests {
    use sottomano_sources::media_playback_audio::mpris_players::{Capabilities, SeekMode};

    use super::*;

    fn titled_player(bus: &str, status: Status) -> Media {
        Media {
            bus: bus.to_owned(),
            owner: OwnerProcess::Unidentified,
            identity: bus.to_owned(),
            desktop_entry: None,
            title: Some("track".to_owned()),
            artist: None,
            album: None,
            url: None,
            status,
            progress: Progress::Untimed,
            art_url: None,
            capabilities: Capabilities::default(),
            shuffle: None,
            loop_mode: None,
            seek_mode: SeekMode::Relative,
        }
    }

    fn reported(group: &mut MediaGroup, players: Vec<Media>, clock: Clock) {
        let _ = group.update(
            MediaMessage::Reading(SourceDelivery::Reading(Ok(players))),
            clock,
        );
    }

    #[test]
    fn each_player_keeps_its_own_session() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        reported(
            &mut group,
            vec![
                titled_player("org.mpris.MediaPlayer2.spotify", Status::Playing),
                titled_player("org.mpris.MediaPlayer2.firefox", Status::Paused),
            ],
            clock,
        );
        let _ = group.update(
            player_message(
                "org.mpris.MediaPlayer2.firefox",
                PlayerMessage::Expansion(ExpansionMessage::Toggle),
            ),
            clock,
        );
        let expanded: Vec<bool> = [
            "org.mpris.MediaPlayer2.firefox",
            "org.mpris.MediaPlayer2.spotify",
        ]
        .iter()
        .map(|bus| group.sessions[*bus].expansion.is_expanded())
        .collect();
        assert_eq!(expanded, [true, false]);
        assert_eq!(group.titled_players().len(), 2);
    }

    #[test]
    fn vanished_player_drops_its_session() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        reported(
            &mut group,
            vec![
                titled_player("org.mpris.MediaPlayer2.spotify", Status::Playing),
                titled_player("org.mpris.MediaPlayer2.firefox", Status::Paused),
            ],
            clock,
        );
        reported(
            &mut group,
            vec![titled_player(
                "org.mpris.MediaPlayer2.spotify",
                Status::Playing,
            )],
            clock,
        );
        assert_eq!(
            group.sessions.keys().collect::<Vec<_>>(),
            ["org.mpris.MediaPlayer2.spotify"]
        );
    }
}
