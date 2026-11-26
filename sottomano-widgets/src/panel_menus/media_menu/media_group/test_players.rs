use sottomano_desktop_sources::{
    media_playback_audio::mpris_players::{
        Capabilities, CoverArtUrl, Media, PlayerSetting, Progress, SeekMode, Status, TrackAlbum,
        TrackSource, TrackTitle,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};

pub(super) fn titled_player(bus: &str, status: Status) -> Media {
    Media {
        bus: bus.to_owned(),
        owner: OwnerProcess::Unidentified,
        identity: bus.to_owned(),
        title: TrackTitle::Titled("track".to_owned()),
        artists: Vec::new(),
        album: TrackAlbum::NoAlbum,
        url: TrackSource::Unlocated,
        status,
        progress: Progress::Untimed,
        art_url: CoverArtUrl::NotAdvertised,
        capabilities: Capabilities::default(),
        queue_order: PlayerSetting::Uncontrollable,
        loop_mode: PlayerSetting::Uncontrollable,
        seek_mode: SeekMode::Relative,
    }
}
