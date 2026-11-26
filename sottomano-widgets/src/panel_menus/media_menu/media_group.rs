use std::{
    collections::{BTreeMap, HashMap},
    time::Instant,
};

use corredo::{
    press_actions::{ConfirmButton, ConfirmPolicy, ConfirmPress},
    value_controls::SeekMessage,
};
use iced::{Element, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    desktop_windows_apps::desktop_entries::DesktopEntries,
    media_playback_audio::{
        lrclib_synced_lyrics::{LyricsQuery, TrackLyrics},
        mpris_players::{Command, CoverArtUrl, Media, PlayerBusOwner, PlayerBusOwners, TrackTitle},
    },
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_indicators::{
        indicator_group_content::{
            GroupEvent, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupTitle,
    },
    icon_catalog::icon_handle::IconHandle,
    widgets::{
        app_window_focus::AppWindowFocus,
        subscribed_snapshot::{CommandOutcome, SubscribedSnapshot},
    },
};

pub mod confirm_button;
pub mod karaoke_lyric_line;
#[cfg(test)]
mod media_group_tests;
mod media_source_subscriptions;
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

const RAISE_PLAYER_HINT: &str = "bring the player's window forward";

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
