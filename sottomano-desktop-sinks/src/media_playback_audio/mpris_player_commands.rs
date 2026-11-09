use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, MediaPlayer2Proxy, PlayerProxy, QueueOrder, micros,
};
use zbus::Connection;

pub async fn control(bus: String, command: Command) -> zbus::Result<()> {
    let session = Connection::session().await?;
    let player = || async {
        PlayerProxy::builder(&session)
            .destination(bus.clone())?
            .build()
            .await
    };
    let root = || async {
        MediaPlayer2Proxy::builder(&session)
            .destination(bus.clone())?
            .build()
            .await
    };
    match command {
        Command::Play => player().await?.play().await,
        Command::Pause => player().await?.pause().await,
        Command::Stop => player().await?.stop().await,
        Command::Next => player().await?.next().await,
        Command::Previous => player().await?.previous().await,
        Command::SetPosition(track, position) => {
            player().await?.set_position(&track, micros(position)).await
        }
        Command::Seek(offset) => player().await?.seek(offset).await,
        Command::SetQueueOrder(queue_order) => {
            player()
                .await?
                .set_shuffle(queue_order == QueueOrder::Shuffled)
                .await
        }
        Command::SetLoopMode(mode) => player().await?.set_loop_status(&mode.to_string()).await,
        Command::Raise => root().await?.raise().await,
        Command::Quit => root().await?.quit().await,
    }
}
