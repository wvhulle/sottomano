use std::time::Duration;

use corredo::transitions::ExpansionState;
use sottomano_desktop_sources::{
    media_playback_audio::mpris_players::{Progress, Status},
    system_protocol_clients::http_json_client,
};

use super::{media_source_subscriptions::PlaybackPace, test_players::titled_player, *};

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
