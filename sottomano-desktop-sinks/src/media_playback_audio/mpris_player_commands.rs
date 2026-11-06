use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, MediaPlayer2ProxyBlocking, PlayerProxyBlocking, micros,
};
use zbus::blocking::Connection as BlockingConnection;

#[must_use]
pub fn control(bus: &str, command: Command) -> bool {
    send_command(bus, command)
        .inspect_err(|e| tracing::warn!("mpris {bus}: {e}"))
        .is_ok()
}

fn send_command(bus: &str, command: Command) -> zbus::Result<()> {
    let connection = BlockingConnection::session()?;
    let player = || {
        PlayerProxyBlocking::builder(&connection)
            .destination(bus.to_owned())?
            .build()
    };
    let root = || {
        MediaPlayer2ProxyBlocking::builder(&connection)
            .destination(bus.to_owned())?
            .build()
    };
    match command {
        Command::Play => player()?.play(),
        Command::Pause => player()?.pause(),
        Command::Stop => player()?.stop(),
        Command::Next => player()?.next(),
        Command::Previous => player()?.previous(),
        Command::SetPosition(track, position) => player()?.set_position(&track, micros(position)),
        Command::Seek(offset) => player()?.seek(offset),
        Command::SetShuffle(shuffle) => player()?.set_shuffle(shuffle),
        Command::SetLoopMode(mode) => player()?.set_loop_status(&mode.to_string()),
        Command::Raise => root()?.raise(),
        Command::Quit => root()?.quit(),
    }
}
