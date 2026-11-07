use sottomano_desktop_sources::media_playback_audio::pipewire_audio::{
    CommandKind, Commands, Target,
};

#[derive(Debug, thiserror::Error)]
#[error("the pipewire loop did not answer the volume change")]
pub struct LevelChangeUnanswered;

pub fn set_volume(
    commands: &Commands,
    target: Target,
    percent: i64,
) -> Result<i64, LevelChangeUnanswered> {
    commands
        .request_level_change(target, CommandKind::SetVolume(percent.clamp(0, 100)))
        .ok_or(LevelChangeUnanswered)
}

#[must_use]
pub fn adjust_volume(commands: &Commands, target: Target, delta: i64) -> Option<i64> {
    commands.request_level_change(target, CommandKind::AdjustVolume(delta))
}

#[must_use]
pub fn toggle_mute(commands: &Commands, target: Target) -> Option<i64> {
    commands.request_level_change(target, CommandKind::ToggleMute)
}
