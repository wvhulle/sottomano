use std::{
    collections::{BTreeMap, HashMap},
    iter,
    time::{Duration, Instant},
};

use corredo::{
    press_actions::{ConfirmButton, ConfirmPolicy, ConfirmPress},
    value_controls::SeekMessage,
};
use iced::{Element, Subscription, Task, time::every};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::desktop_entries::DesktopEntries,
    media_playback_audio::{
        lrclib_synced_lyrics::{LyricsQuery, TrackLyrics},
        mpris_players::{
            Command, CoverArtUrl, Media, PlayerBusOwner, PlayerBusOwners, Progress, Status,
            TrackTitle,
        },
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{
    Config,
    indicator_groups::media_lyrics::{LrclibLookup, MediaLyricsConfig},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_menus::group_source_demand::GroupSourceDemand,
    icon_catalog::icon_handle::IconHandle,
    indicator_groups::{
        indicator_group_content::{
            GroupEvent, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupTitle,
    },
    widgets::{
        app_window_focus::AppWindowFocus,
        subscribed_snapshot::{CommandOutcome, SubscribedSnapshot},
    },
};

pub mod confirm_button;
pub mod karaoke_lyric_line;
mod player_commands;
mod player_failure_group;
mod player_group;
mod seek_bar;
mod session_controls;
mod synced_lyrics_view;
#[cfg(test)]
mod test_players;
mod track_face;
mod track_hover_card;
mod transport_controls;

use self::{
    player_commands::{raise, raise_by_mpris, send},
    player_failure_group::player_failure_group,
    player_group::player_group,
    seek_bar::{SeekBar, seek_by},
    track_face::cover_art_load,
};

const PROGRESS_TICK: Duration = Duration::from_secs(1);
const LYRICS_TICK: Duration = Duration::from_millis(250);
const RAISE_PLAYER_HINT: &str = "bring the player's window forward";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackPace {
    Singing,
    Progressing,
    Still,
}

#[derive(Default)]
pub struct MediaGroup {
    player_owners: SubscribedSnapshot<PlayerBusOwners>,
    player_snapshots: BTreeMap<String, SubscribedSnapshot<Media>>,
    sessions: HashMap<String, PlayerSession>,
    lyrics_snapshots: HashMap<LyricsQuery, SubscribedSnapshot<TrackLyrics>>,
}

struct PlayerSession {
    cover_art: SessionCoverArt,
    seek_bar: SeekBar,
    quit_button: ConfirmButton,
    expansion: IndicatorGroupExpansion,
}

impl Default for PlayerSession {
    fn default() -> Self {
        Self {
            cover_art: SessionCoverArt::NotLoaded,
            seek_bar: SeekBar::default(),
            quit_button: ConfirmButton::new(ConfirmPolicy::ConfirmFirst),
            expansion: IndicatorGroupExpansion::default(),
        }
    }
}

struct CoverArt {
    url: String,
    handle: IconHandle,
}

enum SessionCoverArt {
    NotLoaded,
    Loaded(CoverArt),
}

#[derive(Debug)]
pub enum MediaMessage {
    PlayerOwners(SnapshotDelivery<PlayerBusOwners>),
    PlayerSample {
        bus: String,
        delivery: Box<SnapshotDelivery<Media>>,
    },
    CommandRefused {
        bus: String,
        refusal: Box<zbus::Error>,
    },
    LyricsIndicator {
        query: LyricsQuery,
        delivery: SnapshotDelivery<TrackLyrics>,
    },
    ArtLoaded {
        bus: String,
        url: String,
        handle: IconHandle,
    },
    ProgressAdvanced,
    PlayerFallback(MediaInteraction),
}

#[derive(Debug, Clone)]
pub enum MediaInteraction {
    Player {
        bus: String,
        interaction: PlayerInteraction,
    },
}

#[derive(Debug, Clone)]
pub enum PlayerInteraction {
    Expansion(IndicatorGroupExpansionMessage),
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

fn player_interaction(bus: &str, interaction: PlayerInteraction) -> MediaInteraction {
    MediaInteraction::Player {
        bus: bus.to_owned(),
        interaction,
    }
}

fn player_fallback_message(bus: &str, interaction: PlayerInteraction) -> MediaMessage {
    MediaMessage::PlayerFallback(player_interaction(bus, interaction))
}

fn lifted_to_player<'view>(
    bus: &'view str,
    element: Element<'view, PlayerInteraction>,
) -> Element<'view, GroupEvent> {
    element.map(move |interaction| GroupEvent::from(player_interaction(bus, interaction)))
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
        let owner_samples =
            snapshot_subscription::<PlayerBusOwners>(buses.clone()).map(MediaMessage::PlayerOwners);
        let player_samples: Vec<Subscription<MediaMessage>> = self
            .listed_player_owners()
            .map(|player| {
                snapshot_subscription::<Media>(OnBus {
                    buses: buses.clone(),
                    inputs: player.clone(),
                })
                .with(player.bus.clone())
                .map(|(bus, delivery)| MediaMessage::PlayerSample {
                    bus,
                    delivery: Box::new(delivery),
                })
            })
            .collect();
        let lyrics_lookups = self
            .unfetched_lyrics_queries()
            .filter(|_| lyrics_config.lrclib_lookup == LrclibLookup::Enabled)
            .map(|query| {
                snapshot_subscription::<TrackLyrics>(OnBus {
                    buses: buses.clone(),
                    inputs: query.clone(),
                })
                .with(query)
                .map(|(query, delivery)| MediaMessage::LyricsIndicator { query, delivery })
            });
        let progress_tick = match self.playback_pace() {
            PlaybackPace::Singing => {
                Some(every(LYRICS_TICK).map(|_| MediaMessage::ProgressAdvanced))
            }
            PlaybackPace::Progressing => {
                Some(every(PROGRESS_TICK).map(|_| MediaMessage::ProgressAdvanced))
            }
            PlaybackPace::Still => None,
        }
        .map(|tick| demand.while_headline_shown(tick));
        Subscription::batch(
            iter::once(owner_samples)
                .chain(player_samples)
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

    fn playback_pace(&self) -> PlaybackPace {
        let singing = self.playing_lyrics_queries().any(|query| {
            matches!(
                self.lyrics_snapshots
                    .get(&query)
                    .and_then(SubscribedSnapshot::get),
                Some(TrackLyrics::Synced(_))
            )
        });
        let progressing = self.all_players().any(|media| {
            media.status == Status::Playing && matches!(media.progress, Progress::Timed { .. })
        });
        match (singing, progressing) {
            (true, _) => PlaybackPace::Singing,
            (false, true) => PlaybackPace::Progressing,
            (false, false) => PlaybackPace::Still,
        }
    }

    fn track_lyrics(&self, media: &Media) -> Option<&SubscribedSnapshot<TrackLyrics>> {
        self.lyrics_snapshots.get(&media.lyrics_query()?)
    }

    fn listed_player_owners(&self) -> impl Iterator<Item = &PlayerBusOwner> {
        self.player_owners
            .get()
            .into_iter()
            .flat_map(|player_owners| &player_owners.listed_players)
    }

    fn all_players(&self) -> impl Iterator<Item = &Media> {
        self.player_snapshots
            .values()
            .filter_map(SubscribedSnapshot::get)
    }

    fn player(&self, bus: &str) -> Option<&Media> {
        self.player_snapshots.get(bus)?.get()
    }

    fn titled_players(&self) -> Vec<&Media> {
        let mut titled: Vec<&Media> = self
            .all_players()
            .filter(|media| matches!(media.title, TrackTitle::Titled(_)))
            .collect();
        titled.sort_by(|left, right| left.bus.cmp(&right.bus));
        titled
    }

    fn player_group_title(&self, media: &Media) -> IndicatorGroupTitle {
        let earlier_namesakes = self
            .titled_players()
            .into_iter()
            .take_while(|other| other.bus != media.bus)
            .filter(|other| other.identity == media.identity)
            .count();
        IndicatorGroupTitle::numbered_from_second(media.identity.clone(), earlier_namesakes)
    }

    pub fn update(&mut self, msg: MediaMessage, clock: Clock) -> Action {
        match msg {
            MediaMessage::LyricsIndicator { query, delivery } => {
                self.lyrics_snapshots
                    .entry(query)
                    .or_default()
                    .receive(delivery, clock);
            }
            MediaMessage::ProgressAdvanced => {}
            MediaMessage::PlayerOwners(delivery) => {
                self.player_owners.receive(delivery, clock);
                self.follow_player_owners(clock.instant);
            }
            MediaMessage::PlayerSample { bus, delivery } => {
                let Some(player_snapshot) = self.player_snapshots.get_mut(&bus) else {
                    return Action::Run(Task::none());
                };
                player_snapshot.receive(*delivery, clock);
                self.follow_players(clock.instant);
                return Action::Run(self.art_loads());
            }
            MediaMessage::CommandRefused { bus, refusal } => {
                if let Some(player_snapshot) = self.player_snapshots.get_mut(&bus) {
                    player_snapshot.apply_command(CommandOutcome::Refused { refusal: *refusal });
                }
            }
            MediaMessage::ArtLoaded { bus, url, handle } => {
                let still_shown = self
                    .player(&bus)
                    .is_some_and(|media| {
                        matches!(&media.art_url, CoverArtUrl::Advertised(shown) if *shown == url)
                    });
                if still_shown && let Some(session) = self.sessions.get_mut(&bus) {
                    session.cover_art = SessionCoverArt::Loaded(CoverArt { url, handle });
                }
            }
            MediaMessage::PlayerFallback(fallback_interaction) => {
                return self.interact(fallback_interaction, clock);
            }
        }
        Action::Run(Task::none())
    }

    pub fn interact(&mut self, interaction: MediaInteraction, clock: Clock) -> Action {
        let MediaInteraction::Player {
            bus,
            interaction: player_interaction,
        } = interaction;
        self.update_player(&bus, player_interaction, clock)
    }

    fn follow_player_owners(&mut self, now: Instant) {
        let listed_buses: Vec<String> = self
            .listed_player_owners()
            .map(|player| player.bus.clone())
            .collect();
        self.player_snapshots
            .retain(|bus, _| listed_buses.contains(bus));
        for bus in listed_buses {
            self.player_snapshots.entry(bus).or_default();
        }
        self.follow_players(now);
    }

    fn follow_players(&mut self, now: Instant) {
        let player_snapshots = &self.player_snapshots;
        self.sessions
            .retain(|bus, _| player_snapshots.contains_key(bus));
        let current_queries: Vec<LyricsQuery> =
            self.all_players().filter_map(Media::lyrics_query).collect();
        self.lyrics_snapshots
            .retain(|query, _| current_queries.contains(query));
        for (bus, player_snapshot) in &self.player_snapshots {
            let session = self.sessions.entry(bus.clone()).or_default();
            let Some(media) = player_snapshot.get() else {
                continue;
            };
            session.seek_bar.player_reported(media, now);
            if let SessionCoverArt::Loaded(art) = &session.cover_art
                && !matches!(&media.art_url, CoverArtUrl::Advertised(shown) if *shown == art.url)
            {
                session.cover_art = SessionCoverArt::NotLoaded;
            }
        }
    }

    fn update_player(&mut self, bus: &str, interaction: PlayerInteraction, clock: Clock) -> Action {
        let (Some(media), Some(session)) = (self.player(bus).cloned(), self.sessions.get_mut(bus))
        else {
            return Action::Run(Task::none());
        };
        let task = match interaction {
            PlayerInteraction::Expansion(IndicatorGroupExpansionMessage::HandleScrolled(delta)) => {
                seek_by(session, &media, SeekMessage::Scroll(delta), clock.instant)
            }
            PlayerInteraction::Expansion(expansion) => {
                session.expansion.update(expansion);
                Task::none()
            }
            PlayerInteraction::Command(Command::Raise) => return raise(&media),
            PlayerInteraction::RaiseByMpris => raise_by_mpris(&media),
            PlayerInteraction::Command(command) => send(&media, command),
            PlayerInteraction::Seek(seek) => seek_by(session, &media, seek, clock.instant),
            PlayerInteraction::CancelQuit => {
                session.quit_button.disarm();
                Task::none()
            }
            PlayerInteraction::Quit => match session.quit_button.press() {
                ConfirmPress::Armed => Task::none(),
                ConfirmPress::Fire => send(&media, Command::Quit),
            },
        };
        Action::Run(task)
    }

    fn art_loads(&self) -> Task<MediaMessage> {
        Task::batch(
            self.player_snapshots
                .iter()
                .filter_map(|(bus, player_snapshot)| {
                    cover_art_load(player_snapshot.get()?, self.sessions.get(bus)?)
                }),
        )
    }

    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        desktop_entries: &'view DesktopEntries,
        clock: Clock,
    ) -> impl Iterator<Item = IndicatorGroupContent<'view>> {
        self.player_snapshots
            .iter()
            .filter_map(move |(bus, player_snapshot)| {
                let session = self.sessions.get(bus)?;
                if let Some(read_failure) = player_snapshot.failure() {
                    return Some(player_failure_group(bus, read_failure, session));
                }
                let media = player_snapshot
                    .get()
                    .filter(|media| matches!(media.title, TrackTitle::Titled(_)))?;
                player_group(
                    config,
                    clock,
                    self.player_group_title(media),
                    player_snapshot,
                    session,
                    self.track_lyrics(media),
                    desktop_entries,
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use corredo::transitions::ExpansionState;
    use sottomano_desktop_sources::system_protocol_clients::http_json_client;

    use super::{test_players::titled_player, *};

    fn reported(group: &mut MediaGroup, players: Vec<Media>, clock: Clock) {
        let player_owners = players
            .iter()
            .enumerate()
            .map(|(position, media)| PlayerBusOwner {
                bus: media.bus.clone(),
                owner: format!(":1.{position}"),
            })
            .collect();
        let _ = group.update(
            MediaMessage::PlayerOwners(SnapshotDelivery::live_push(Ok(PlayerBusOwners {
                listed_players: player_owners,
            }))),
            clock,
        );
        for media in players {
            let _ = group.update(
                MediaMessage::PlayerSample {
                    bus: media.bus.clone(),
                    delivery: Box::new(SnapshotDelivery::live_push(Ok(media))),
                },
                clock,
            );
        }
    }

    #[test]
    fn a_player_whose_first_read_fails_shows_its_failure_in_place() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        reported(
            &mut group,
            vec![titled_player(
                "org.mpris.MediaPlayer2.spotify",
                Status::Playing,
            )],
            clock,
        );
        let _ = group.update(
            MediaMessage::PlayerOwners(SnapshotDelivery::live_push(Ok(PlayerBusOwners {
                listed_players: vec![
                    PlayerBusOwner {
                        bus: "org.mpris.MediaPlayer2.ShairportSync".to_owned(),
                        owner: ":1.7".to_owned(),
                    },
                    PlayerBusOwner {
                        bus: "org.mpris.MediaPlayer2.spotify".to_owned(),
                        owner: ":1.0".to_owned(),
                    },
                ],
            }))),
            clock,
        );
        let _ = group.update(
            MediaMessage::PlayerSample {
                bus: "org.mpris.MediaPlayer2.ShairportSync".to_owned(),
                delivery: Box::new(SnapshotDelivery::live_push(Err(zbus::Error::Failure(
                    "no reply".to_owned(),
                )))),
            },
            clock,
        );
        let config = Config::default();
        let desktop_entries = DesktopEntries::default();
        let group_titles: Vec<String> = group
            .group_contents(&config, &desktop_entries, clock)
            .map(|group_content| group_content.group_title.to_string())
            .collect();
        assert_eq!(
            group_titles,
            ["ShairportSync", "org.mpris.MediaPlayer2.spotify"]
        );
    }

    #[test]
    fn a_refused_command_shows_in_place_of_the_player() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        reported(
            &mut group,
            vec![titled_player(
                "org.mpris.MediaPlayer2.spotify",
                Status::Playing,
            )],
            clock,
        );
        let _ = group.update(
            MediaMessage::CommandRefused {
                bus: "org.mpris.MediaPlayer2.spotify".to_owned(),
                refusal: Box::new(zbus::Error::Failure("not playing".to_owned())),
            },
            clock,
        );
        assert!(
            group.player_snapshots["org.mpris.MediaPlayer2.spotify"]
                .failure()
                .is_some()
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
        let _ = group.interact(
            player_interaction(
                "org.mpris.MediaPlayer2.firefox",
                PlayerInteraction::Expansion(IndicatorGroupExpansionMessage::Toggle),
            ),
            clock,
        );
        let expansion_states: Vec<ExpansionState> = [
            "org.mpris.MediaPlayer2.firefox",
            "org.mpris.MediaPlayer2.spotify",
        ]
        .iter()
        .map(|bus| group.sessions[*bus].expansion.state)
        .collect();
        assert_eq!(
            expansion_states,
            [ExpansionState::Expanded, ExpansionState::Collapsed]
        );
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
            .map(|media| group.player_group_title(media).to_string())
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
            title: TrackTitle::Titled(title.to_owned()),
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
            MediaMessage::LyricsIndicator {
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
        assert_eq!(group.playback_pace(), PlaybackPace::Singing);
        assert_eq!(group.unfetched_lyrics_queries().count(), 0);
        reported(&mut group, vec![singing_player("High Water", clock)], clock);
        assert!(group.lyrics_snapshots.is_empty());
        assert_ne!(group.playback_pace(), PlaybackPace::Singing);
    }

    #[test]
    fn a_failed_attempt_shows_while_the_lookup_keeps_retrying() {
        let clock = Clock::real();
        let mut group = MediaGroup::default();
        let low_tide = singing_player("Low Tide", clock);
        reported(&mut group, vec![low_tide.clone()], clock);
        let _ = group.update(
            MediaMessage::LyricsIndicator {
                query: low_tide
                    .lyrics_query()
                    .expect("a timed, titled track has a query"),
                delivery: SnapshotDelivery::single_read(Err(http_json_client::Error::status(
                    503,
                    http_json_client::ResponseBody::NotJson,
                ))),
            },
            clock,
        );
        assert_eq!(group.unfetched_lyrics_queries().count(), 1);
        assert!(
            group
                .track_lyrics(&low_tide)
                .is_some_and(|lyrics_snapshot| lyrics_snapshot.failure().is_some())
        );
    }
}
