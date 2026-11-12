use std::{env, future::ready, path::PathBuf};

use futures::{Stream, stream};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::ChangeTriggerStreams,
    screen_brightness_color::solar_switch_schedule::ColorMode,
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        shell_command_output,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct DarkMode {
    pub color_mode: ColorMode,
}

impl SnapshotSource for DarkMode {
    type Error = shell_command_output::Error;
    type StreamInputs = ();

    const NAME: &'static str = "Dark mode";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::program_availability("umbra")).while_available(
            || {
                SnapshotDelivery::live_push_deliveries((), |()| {
                    ready(Ok(changes(
                        mode_file().map(WatchTarget::File).into_iter().collect(),
                    )
                    .triggered_rereads(Self::read)))
                })
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

impl DarkMode {
    async fn read() -> Result<Self, shell_command_output::Error> {
        let mode = shell_command_output::output("umbra", &["get"]).await?;
        Ok(DarkMode {
            color_mode: match mode.trim() {
                "dark" => ColorMode::Dark,
                _ => ColorMode::Light,
            },
        })
    }
}

fn mode_file() -> Option<PathBuf> {
    let state_home = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".local/state")))?;
    Some(state_home.join("umbra/mode.env"))
}
