use std::{env, future::ready, path::PathBuf};

use futures::{Stream, stream};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, command,
    directory_changes::{WatchTarget, changes},
    pushed_delivery_stream,
    stream_operators::{StreamOperators, read_once_then_per_burst},
    sun_schedule::ColorMode,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Umbra {
    pub dark: bool,
}

impl ReadingSource for Umbra {
    type Error = command::Error;
    type StreamInputs = ();

    const NAME: &'static str = "umbra";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(command::exists("umbra")).while_held(
            || {
                pushed_delivery_stream(ready(Ok(read_once_then_per_burst(
                    changes(mode_file().map(WatchTarget::File).into_iter().collect()),
                    Self::read,
                ))))
            },
            SourceDelivery::Absent,
        )
    }
}

impl Umbra {
    async fn read() -> Result<Self, command::Error> {
        let mode = command::output("umbra", &["get"]).await?;
        Ok(Umbra {
            dark: mode.trim() == "dark",
        })
    }
}

fn mode_file() -> Option<PathBuf> {
    let state_home = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".local/state")))?;
    Some(state_home.join("umbra/mode.env"))
}

pub fn toggle_mode() -> Result<(), command::Error> {
    command::blocking::output("umbra", &["toggle"]).map(drop)
}

#[must_use]
pub fn set_mode(mode: ColorMode) -> bool {
    command::blocking::run("umbra", &["set", mode.umbra_name()])
}
