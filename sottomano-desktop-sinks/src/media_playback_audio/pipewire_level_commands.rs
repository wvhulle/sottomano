use sottomano_desktop_sources::media_playback_audio::pipewire_audio::{
    CommandKind, Commands, Target,
};
use tokio::task::JoinError;

#[derive(Debug, thiserror::Error)]
pub enum VolumeChangeFailure {
    #[error("the pipewire loop did not answer the volume change")]
    Unanswered,
    #[error("the volume change task did not finish: {0}")]
    WorkerStopped(#[from] JoinError),
}

pub fn set_volume(
    commands: &Commands,
    target: Target,
    percent: i64,
) -> Result<i64, VolumeChangeFailure> {
    commands
        .request_level_change(target, CommandKind::SetVolume(percent.clamp(0, 100)))
        .ok_or(VolumeChangeFailure::Unanswered)
}

#[must_use]
pub fn adjust_volume(commands: &Commands, target: Target, delta: i64) -> Option<i64> {
    commands.request_level_change(target, CommandKind::AdjustVolume(delta))
}

#[must_use]
pub fn toggle_mute(commands: &Commands, target: Target) -> Option<i64> {
    commands.request_level_change(target, CommandKind::ToggleMute)
}
