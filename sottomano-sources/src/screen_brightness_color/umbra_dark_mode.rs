use std::{env, future::ready, path::PathBuf};

use futures::{Stream, stream};

use crate::{
    SnapshotDelivery, SnapshotSource,
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

impl SnapshotSource for DarkMode {
    type Error = shell_command_output::Error;
    type StreamInputs = ();

    const NAME: &'static str = "umbra";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("umbra")).while_held(
            || {
                SnapshotDelivery::live_push_deliveries(ready(Ok(read_once_then_per_burst(
                    changes(mode_file().map(WatchTarget::File).into_iter().collect()),
                    Self::read,
                ))))
            },
            SnapshotDelivery::SourceUnavailable,
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
