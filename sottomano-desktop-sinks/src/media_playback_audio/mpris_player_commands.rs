use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, MediaPlayer2ProxyBlocking, PlayerProxyBlocking, QueueOrder, micros,
};
use zbus::blocking::Connection as BlockingConnection;

pub fn control(bus: &str, command: Command) -> zbus::Result<()> {
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
        Command::SetQueueOrder(queue_order) => {
            player()?.set_shuffle(queue_order == QueueOrder::Shuffled)
        }
        Command::SetLoopMode(mode) => player()?.set_loop_status(&mode.to_string()),
        Command::Raise => root()?.raise(),
        Command::Quit => root()?.quit(),
    }
}
