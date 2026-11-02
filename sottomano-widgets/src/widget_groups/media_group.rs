use std::{
    borrow::Cow,
    collections::HashMap,
    iter,
    time::{Duration, Instant},
};

use iced::{Element, Subscription, Task, time::every, widget::Row};
use sottomano_config::{Config, widget_groups::media_lyrics::MediaLyricsConfig};
use sottomano_sources::{
    SnapshotDelivery,
    blocking_background_work::blocking,
    desktop_windows_apps::niri_window_placement::WindowOwner,
    media_playback_audio::{
        lrclib_synced_lyrics::{LyricsQuery, TrackLyrics},
        mpris_players::{self, Command, Media, Progress, Status},
    },
    network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OwnerProcess},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    desktop_index::DesktopIndex,
    group_items::{
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        reading_display_state::ReadingDisplayState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget, PressHints,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
        hoverable,
    },
    widget_groups::{
        media_group::confirm_button::{ConfirmButton, Press},
        widget_group_content::{
            GroupEvent, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{
            WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupInstance, WidgetGroupTitle,
        },
    },
    widgets::{
        app_window_focus::AppWindowFocus,
        icon_handle::IconHandle,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub mod confirm_button;
pub mod karaoke_lyric_line;
mod seek_bar;
mod session_controls;
mod synced_lyrics_view;
mod track_face;
mod track_hover_card;
mod transport_controls;

use self::{
    seek_bar::{SeekBar, SeekMessage},
    session_controls::{SessionControls, SessionMessages},
    synced_lyrics_view::{lyrics_item, lyrics_section},
    track_face::{player_item, track_item},
    track_hover_card::{playback_section, search_section, track_section},
    transport_controls::TransportControls,
};

const PROGRESS_TICK: Duration = Duration::from_secs(1);
const LYRICS_TICK: Duration = Duration::from_millis(250);
const RAISE_PLAYER_HINT: &str = "bring the player's window forward";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaTarget {
    Player,
    Track,
    Lyrics,
    Seek,
    Shuffle,
    Previous,
    PlayPause,
    Next,
    Loop,
    Transport,
    Stop,
    Quit,
    SessionControls,
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
    players: LatestSnapshot<Vec<Media>, HidesFetchFailures>,
    sessions: HashMap<String, PlayerSession>,
    lyrics_snapshots: HashMap<LyricsQuery, LatestSnapshot<TrackLyrics>>,
}

struct PlayerSession {
    cover_art: Option<CoverArt>,
    seek_bar: SeekBar,
    quit_button: ConfirmButton,
    expansion: WidgetGroupExpansion,
}

impl Default for PlayerSession {
    fn default() -> Self {
        Self {
            cover_art: None,
            seek_bar: SeekBar::default(),
            quit_button: ConfirmButton::new(true),
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

struct CoverArt {
    url: String,
    handle: IconHandle,
}

#[derive(Debug, Clone)]
pub enum MediaMessage {
    Reading(SnapshotDelivery<Vec<Media>>),
    LyricsReading {
        query: LyricsQuery,
        delivery: SnapshotDelivery<TrackLyrics>,
    },
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
    Expansion(WidgetGroupExpansionMessage),
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

fn player_group_identity(media: &Media, bus: &str) -> WidgetGroupIdentity {
    WidgetGroupIdentity {
        family: WidgetGroupFamily::Media,
        instance: WidgetGroupInstance::Session {
            durable: media.identity.clone().into(),
            session: bus.to_owned().into(),
        },
    }
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

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        lyrics_config: &MediaLyricsConfig,
        demand: &GroupSourceDemand,
    ) -> Subscription<MediaMessage> {
        let readings =
            snapshot_subscription::<Vec<Media>>(buses.clone()).map(MediaMessage::Reading);
        let lyrics_lookups = self
            .unfetched_lyrics_queries()
            .filter(|_| lyrics_config.lrclib_lookup)
            .map(|query| {
                snapshot_subscription::<TrackLyrics>(OnlineSourceInputs {
                    buses: buses.clone(),
                    source_inputs: query.clone(),
                })
                .with(query)
                .map(|(query, delivery)| MediaMessage::LyricsReading { query, delivery })
            });
        let progress_tick = if self.is_singing() {
            Some(every(LYRICS_TICK).map(|_| MediaMessage::ProgressAdvanced))
        } else if self.is_progressing() {
            Some(every(PROGRESS_TICK).map(|_| MediaMessage::ProgressAdvanced))
        } else {
            None
        }
        .map(|tick| demand.while_any_reading_shown(tick));
        Subscription::batch(
            iter::once(readings)
                .chain(lyrics_lookups)
                .chain(progress_tick),
        )
    }

    fn playing_lyrics_queries(&self) -> impl Iterator<Item = LyricsQuery> {
        self.all_players()
            .filter(|media| media.status == Status::Playing)
            .filter_map(Media::lyrics_query)
    }

    fn unfetched_lyrics_queries(&self) -> impl Iterator<Item = LyricsQuery> {
        self.playing_lyrics_queries().filter(|query| {
            self.lyrics_snapshots
                .get(query)
                .is_none_or(|lyrics_snapshot| lyrics_snapshot.get().is_none())
        })
    }

    fn is_singing(&self) -> bool {
        self.playing_lyrics_queries().any(|query| {
            matches!(
                self.lyrics_snapshots
                    .get(&query)
                    .and_then(LatestSnapshot::get),
                Some(TrackLyrics::Synced(_))
            )
        })
    }

    fn lyrics_of(&self, media: &Media) -> Option<&LatestSnapshot<TrackLyrics>> {
        self.lyrics_snapshots.get(&media.lyrics_query()?)
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

    fn group_title_of(&self, media: &Media) -> WidgetGroupTitle {
        let earlier_namesakes = self
            .titled_players()
            .into_iter()
            .take_while(|other| other.bus != media.bus)
            .filter(|other| other.identity == media.identity)
            .count();
        WidgetGroupTitle::numbered_from_second(media.identity.clone(), earlier_namesakes)
    }

    pub fn update(&mut self, msg: MediaMessage, clock: Clock) -> Action {
        match msg {
            MediaMessage::LyricsReading { query, delivery } => {
                self.lyrics_snapshots
                    .entry(query)
                    .or_default()
                    .receive(delivery, clock);
            }
            MediaMessage::ProgressAdvanced => {}
            MediaMessage::Reading(delivery) => {
                self.players.receive(delivery, clock);
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
        let current_queries: Vec<LyricsQuery> =
            players.iter().filter_map(Media::lyrics_query).collect();
        self.lyrics_snapshots
            .retain(|query, _| current_queries.contains(query));
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
            PlayerMessage::Expansion(WidgetGroupExpansionMessage::HandleScrolled(delta)) => {
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
    pub fn hover_card_for(
        &self,
        target: &PlayerTarget,
        index: &DesktopIndex,
        clock: Clock,
    ) -> Option<HoverCard> {
        let media = self.player(&target.bus)?;
        match target.control {
            MediaTarget::Player => self.players.hover_card_with(HoverCardBody::dictionary([
                LabeledValue::new("Player", media.identity.clone()),
                LabeledValue::new("State", media.status.label()),
            ])),
            MediaTarget::Track => self.players.hover_card_with(HoverCardBody::sections([
                track_section(media, index),
                playback_section(media, clock.instant),
                search_section(media, index),
            ])),
            MediaTarget::Lyrics => self.lyrics_hover_card(media, clock),
            MediaTarget::Seek
            | MediaTarget::Shuffle
            | MediaTarget::Previous
            | MediaTarget::PlayPause
            | MediaTarget::Next
            | MediaTarget::Loop
            | MediaTarget::Transport
            | MediaTarget::Stop
            | MediaTarget::Quit
            | MediaTarget::SessionControls => {
                self.players.hover_card_with(HoverCardBody::sections([]))
            }
        }
    }

    fn lyrics_hover_card(&self, media: &Media, clock: Clock) -> Option<HoverCard> {
        let lyrics_snapshot = self.lyrics_of(media)?;
        let Some(track_lyrics) = lyrics_snapshot.get() else {
            return lyrics_snapshot.hover_card(clock);
        };
        let played = media.progress.played(media.status, clock.instant)?;
        lyrics_snapshot.hover_card_with(HoverCardBody::sections([lyrics_section(
            track_lyrics,
            played,
        )]))
    }

    pub fn group_contents<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> impl Iterator<Item = WidgetGroupContent<'a, MediaMessage>> {
        self.titled_players().into_iter().filter_map(move |media| {
            let session = self.sessions.get(&media.bus)?;
            let group_title = self.group_title_of(media);
            Some(player_group(
                config,
                clock,
                group_title,
                media,
                session,
                self.lyrics_of(media),
            ))
        })
    }
}

fn session_buttons_item<'a>(
    config: &'a Config,
    media: &'a Media,
    session: &'a PlayerSession,
) -> Option<GroupItem<'a, MediaMessage>> {
    let bus = media.bus.as_str();
    (!session_controls(config, media, session, 1.0).is_empty()).then(|| {
        GroupItem::composed(
            ComposedGroupItemFace {
                name: "Session".into(),
                icon: GroupItemIcon::Absent,
            },
            GroupItemHoverArea::ByChildren {
                row_target: PlayerTarget::of(bus, MediaTarget::SessionControls).into(),
            },
            GroupItemValue::drawn(move |reveal| {
                lifted_to_player(bus, session_controls(config, media, session, reveal).into())
            }),
        )
    })
}

fn player_group<'a>(
    config: &'a Config,
    clock: Clock,
    group_title: WidgetGroupTitle,
    media: &'a Media,
    session: &'a PlayerSession,
    lyrics_snapshot: Option<&'a LatestSnapshot<TrackLyrics>>,
) -> WidgetGroupContent<'a, MediaMessage> {
    let bus = media.bus.as_str();
    let seekable = SeekBar::seekable(media, clock.instant);
    let gauge_fraction = media
        .progress
        .fraction(media.status, clock.instant)
        .filter(|_| !(seekable && session.expansion.is_expanded()));
    let seek = seekable.then(|| {
        GroupItem::composed(
            ComposedGroupItemFace {
                name: "Position".into(),
                icon: GroupItemIcon::IconGlyph {
                    glyph: Phosphor::HourglassMedium(Weight::Regular).into(),
                    state: ReadingDisplayState::Idle,
                },
            },
            GroupItemHoverArea::ByChildren {
                row_target: PlayerTarget::of(bus, MediaTarget::Seek).into(),
            },
            GroupItemValue::drawn(move |_| {
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
                        GroupEvent::HoverCard,
                    ),
                )
            }),
        )
    });
    let transport = GroupItem::composed(
        ComposedGroupItemFace {
            name: "Playback".into(),
            icon: GroupItemIcon::Absent,
        },
        GroupItemHoverArea::ByChildren {
            row_target: PlayerTarget::of(bus, MediaTarget::Transport).into(),
        },
        GroupItemValue::drawn(move |reveal| {
            lifted_to_player(
                bus,
                TransportControls::new(
                    config,
                    media,
                    reveal,
                    |command| GroupEvent::Group(PlayerMessage::Command(command)),
                    GroupEvent::HoverCard,
                )
                .into(),
            )
        }),
    );
    let session_buttons = session_buttons_item(config, media, session);
    WidgetGroupContent::expandable_instance(
        player_group_identity(media, bus),
        group_title,
        &session.expansion,
        move |expansion| player_message(bus, PlayerMessage::Expansion(expansion)),
        [
            Some(player_item(config, media, gauge_fraction)),
            Some(track_item(
                config,
                media,
                session.cover_art.as_ref().map(|art| &art.handle),
            )),
            lyrics_snapshot.and_then(|lyrics_snapshot| {
                lyrics_item(config, media, lyrics_snapshot, clock.instant)
            }),
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
            hover_card: GroupEvent::HoverCard,
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
    use sottomano_sources::{
        media_playback_audio::mpris_players::{Capabilities, SeekMode},
        system_protocol_clients::http_json_client,
    };

    use super::*;

    fn titled_player(bus: &str, status: Status) -> Media {
        Media {
            bus: bus.to_owned(),
            owner: OwnerProcess::Unidentified,
            identity: bus.to_owned(),
            desktop_entry: None,
            title: Some("track".to_owned()),
            artists: Vec::new(),
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
            MediaMessage::Reading(SnapshotDelivery::live_push(Ok(players))),
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
                PlayerMessage::Expansion(WidgetGroupExpansionMessage::Toggle),
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
    fn each_player_titles_its_own_group() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        let named_player = |bus: &str, identity: &str| Media {
            identity: identity.to_owned(),
            ..titled_player(bus, Status::Playing)
        };
        reported(
            &mut group,
            vec![
                named_player("org.mpris.MediaPlayer2.spotify", "Spotify"),
                named_player("org.mpris.MediaPlayer2.firefox.instance1", "Firefox"),
                named_player("org.mpris.MediaPlayer2.firefox.instance2", "Firefox"),
            ],
            clock,
        );
        let titles: Vec<String> = group
            .titled_players()
            .into_iter()
            .map(|media| group.group_title_of(media).to_string())
            .collect();
        assert_eq!(titles, ["Firefox", "Firefox 2", "Spotify"]);
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

    fn singing_player(title: &str, clock: Clock) -> Media {
        Media {
            title: Some(title.to_owned()),
            artists: vec!["The Harbour Lights".to_owned()],
            progress: Progress::Timed {
                position: Duration::from_secs(15),
                sampled_at: clock.instant,
                length: Duration::from_secs(241),
                rate: 1.0,
            },
            ..titled_player("org.mpris.MediaPlayer2.spotify", Status::Playing)
        }
    }

    fn lyrics_delivered(group: &mut MediaGroup, media: &Media, clock: Clock) {
        let _ = group.update(
            MediaMessage::LyricsReading {
                query: media
                    .lyrics_query()
                    .expect("a timed, titled track has a query"),
                delivery: SnapshotDelivery::single_read(Ok(TrackLyrics::synced_from_lrc(
                    "[00:10.00]Low tide, low tide",
                ))),
            },
            clock,
        );
    }

    #[test]
    fn delivered_lyrics_follow_the_track_and_drop_on_a_track_change() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        let low_tide = singing_player("Low Tide", clock);
        reported(&mut group, vec![low_tide.clone()], clock);
        assert_eq!(group.unfetched_lyrics_queries().count(), 1);
        lyrics_delivered(&mut group, &low_tide, clock);
        assert!(group.is_singing());
        assert_eq!(group.unfetched_lyrics_queries().count(), 0);
        reported(&mut group, vec![singing_player("High Water", clock)], clock);
        assert!(group.lyrics_snapshots.is_empty());
        assert!(!group.is_singing());
    }

    #[test]
    fn a_failed_attempt_shows_while_the_lookup_keeps_retrying() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        let low_tide = singing_player("Low Tide", clock);
        reported(&mut group, vec![low_tide.clone()], clock);
        let _ = group.update(
            MediaMessage::LyricsReading {
                query: low_tide
                    .lyrics_query()
                    .expect("a timed, titled track has a query"),
                delivery: SnapshotDelivery::single_read(Err(http_json_client::Error::status(
                    503, None,
                ))),
            },
            clock,
        );
        assert_eq!(group.unfetched_lyrics_queries().count(), 1);
        assert!(
            group
                .lyrics_of(&low_tide)
                .is_some_and(|lyrics_snapshot| lyrics_snapshot.failure().is_some())
        );
    }
}
