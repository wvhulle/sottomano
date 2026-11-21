use std::{convert::identity, env, thread, time::Duration};

use iced::Task;
use sottomano_desktop_sinks::media_playback_audio::pipewire_level_commands;
use sottomano_desktop_sources::{
    blocking_background_work::blocking,
    media_playback_audio::pipewire_audio::{Commands, Target},
};
use tokio::task::spawn_blocking;

use super::{AudioMessage, ControlKind};
use crate::group_indicators::slider_control::{ControlMessage, as_percent, percent_f};

#[derive(Clone)]
pub(super) enum PipewireCommandLink {
    Disconnected,
    Connected(Commands),
}

impl PipewireCommandLink {
    pub(super) fn write_volume(
        &self,
        kind: ControlKind,
        target: Target,
        value: f32,
    ) -> Task<AudioMessage> {
        let PipewireCommandLink::Connected(commands) = self.clone() else {
            return Task::none();
        };
        let volume_write = spawn_blocking(move || {
            slow_write_delay();
            pipewire_level_commands::set_volume(&commands, target, as_percent(value)).map(drop)
        });
        ControlMessage::commit("audio set volume", value, async move {
            volume_write
                .await
                .map_err(pipewire_level_commands::VolumeChangeFailure::from)
                .and_then(identity)
        })
        .map(move |control_message| AudioMessage::Control(kind, control_message))
    }

    pub(super) fn nudge_volume(
        &self,
        kind: ControlKind,
        target: Target,
        delta: f32,
    ) -> Task<AudioMessage> {
        let PipewireCommandLink::Connected(commands) = self.clone() else {
            return Task::none();
        };
        let step = as_percent(delta);
        Task::perform(
            blocking("audio adjust volume", move || {
                slow_write_delay();
                pipewire_level_commands::adjust_volume(&commands, target, step)
            }),
            move |resolved| {
                AudioMessage::Control(
                    kind,
                    resolved.map_or(ControlMessage::AdjustFailed, |percent| {
                        ControlMessage::Adjusted(percent_f(percent))
                    }),
                )
            },
        )
    }

    pub(super) fn toggle_mute(&self, target: Target) -> Task<AudioMessage> {
        match self.clone() {
            PipewireCommandLink::Connected(commands) => {
                Task::future(blocking("audio toggle mute", move || {
                    pipewire_level_commands::toggle_mute(&commands, target)
                }))
                .discard()
            }
            PipewireCommandLink::Disconnected => Task::none(),
        }
    }
}

fn slow_write_delay() {
    if let Ok(raw) = env::var("SOTTOMANO_SLOW_WRITE_MS")
        && let Ok(ms) = raw.parse::<u64>()
    {
        thread::sleep(Duration::from_millis(ms));
    }
}
