use std::{env, future::ready, path::PathBuf};

use futures::{Stream, stream};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    screen_brightness_color::solar_switch_schedule::ColorMode,
    stream_operators::{StreamOperators, read_once_then_per_burst},
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        shell_command_output,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct DarkMode {
    pub dark: bool,
}

impl ReadingSource for DarkMode {
    type Error = shell_command_output::Error;
    type StreamInputs = ();

    const NAME: &'static str = "umbra";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("umbra")).while_held(
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

impl DarkMode {
    async fn read() -> Result<Self, shell_command_output::Error> {
        let mode = shell_command_output::output("umbra", &["get"]).await?;
        Ok(DarkMode {
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

pub fn toggle_mode() -> Result<(), shell_command_output::Error> {
    shell_command_output::blocking::output("umbra", &["toggle"]).map(drop)
}

#[must_use]
pub fn set_mode(mode: ColorMode) -> bool {
    shell_command_output::blocking::run("umbra", &["set", mode.umbra_name()])
}
