use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use super::{
    mpris_property_decoding::{NO_TRACK, controllable_setting, declared_queue_order, parsed},
    *,
};
use crate::system_protocol_clients::dbus_desktop_buses::Properties;

fn owned<'value>(value: impl Into<Value<'value>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}

fn all_player_capabilities(control: bool) -> Properties {
    Properties::from([
        ("CanControl".to_owned(), owned(control)),
        ("CanPlay".to_owned(), owned(true)),
        ("CanPause".to_owned(), owned(true)),
        ("CanSeek".to_owned(), owned(true)),
        ("CanGoNext".to_owned(), owned(true)),
        ("CanGoPrevious".to_owned(), owned(true)),
        ("Shuffle".to_owned(), owned(true)),
        ("LoopStatus".to_owned(), owned("Track")),
    ])
}

#[test]
fn uncontrollable_player_declares_no_player_capabilities() {
    let player = all_player_capabilities(false);
    let root = Properties::from([
        ("CanRaise".to_owned(), owned(true)),
        ("CanQuit".to_owned(), owned(true)),
    ]);
    assert_eq!(
        Capabilities::declared(&player, &root),
        Capabilities {
            raise: CommandOffer::Offered,
            quit: CommandOffer::Offered,
            ..Capabilities::default()
        }
    );
    assert_eq!(
        controllable_setting(&player, declared_queue_order(&player)),
        PlayerSetting::Uncontrollable
    );
}

#[test]
fn controllable_player_exposes_declared_settings() {
    let player = all_player_capabilities(true);
    assert_eq!(
        controllable_setting(&player, parsed(&player, "LoopStatus")),
        PlayerSetting::Controllable(LoopMode::Track)
    );
    assert_eq!(
        controllable_setting(&player, declared_queue_order(&player)),
        PlayerSetting::Controllable(QueueOrder::Shuffled)
    );
}

#[test]
fn missing_can_pause_is_false() {
    let player = Properties::from([
        ("CanControl".to_owned(), owned(true)),
        ("CanPlay".to_owned(), owned(true)),
    ]);
    let capabilities = Capabilities::declared(&player, &Properties::new());
    assert_eq!(capabilities.play, CommandOffer::Offered);
    assert_eq!(capabilities.pause, CommandOffer::NotOffered);
}

#[test]
fn loop_mode_cycles_through_every_mode() {
    assert_eq!(LoopMode::Off.cycled(), LoopMode::Playlist);
    assert_eq!(LoopMode::Playlist.cycled(), LoopMode::Track);
    assert_eq!(LoopMode::Track.cycled(), LoopMode::Off);
}

#[test]
fn loop_mode_round_trips_through_its_spec_name() {
    [LoopMode::Off, LoopMode::Playlist, LoopMode::Track]
        .into_iter()
        .for_each(|mode| assert_eq!(mode.to_string().parse::<LoopMode>().ok(), Some(mode)));
    assert!("Shuffled".parse::<LoopMode>().is_err());
}

#[test]
fn seek_mode_is_relative_without_a_real_track() {
    assert_eq!(SeekMode::declared(&Properties::new()), SeekMode::Relative);
    let no_track = Properties::from([(
        "mpris:trackid".to_owned(),
        owned(ObjectPath::try_from(NO_TRACK).unwrap()),
    )]);
    assert_eq!(SeekMode::declared(&no_track), SeekMode::Relative);
    let track = ObjectPath::try_from("/org/mpd/Tracks/7").unwrap();
    let declared = Properties::from([("mpris:trackid".to_owned(), owned(track.clone()))]);
    assert_eq!(
        SeekMode::declared(&declared),
        SeekMode::Absolute(track.into())
    );
    let spotify_track = "/com/spotify/track/4Rrhbvf23LWmr4IxwOBGrH";
    let declared_as_text = Properties::from([("mpris:trackid".to_owned(), owned(spotify_track))]);
    assert_eq!(
        SeekMode::declared(&declared_as_text),
        SeekMode::Absolute(ObjectPath::try_from(spotify_track).unwrap().into())
    );
}

#[test]
fn played_time_advances_at_the_declared_rate() {
    let sampled_at = Instant::now();
    let progress = Progress::Timed {
        position: Duration::from_secs(10),
        sampled_at,
        length: Duration::from_secs(100),
        rate: 2.0,
    };
    let later = sampled_at + Duration::from_secs(5);
    assert_eq!(
        progress.played(Status::Playing, later),
        Some(Duration::from_secs(20))
    );
    assert_eq!(
        progress.played(Status::Paused, later),
        Some(Duration::from_secs(10))
    );
}
