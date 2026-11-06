use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sottomano_widgets::clock::Clock;

use crate::showcase_message::ShowcaseMessage;

pub const LIVE_INTERVAL: Duration = Duration::from_millis(1500);
const SHOWCASE_EPOCH_UNIX_SECONDS: u64 = 1_781_517_600;

#[must_use]
pub fn showcase_epoch() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(SHOWCASE_EPOCH_UNIX_SECONDS)
}

#[must_use]
pub fn sample(clock: Clock) -> u32 {
    let elapsed = clock
        .wall
        .duration_since(showcase_epoch())
        .unwrap_or(Duration::ZERO);
    u32::try_from(elapsed.as_millis() / LIVE_INTERVAL.as_millis()).unwrap_or(u32::MAX)
}

pub fn cycled<T: Copy>(values: &[T], tick: u32) -> T {
    values[usize::try_from(tick).unwrap_or_default() % values.len()]
}

pub fn unchanging(_clock: Clock) -> Vec<ShowcaseMessage> {
    Vec::new()
}
pub mod app_launcher;
pub mod appearance_presets;
pub mod audio;
pub mod caldav;
pub mod ci_runner_tasks;
pub mod claude_usage;
pub mod compute;
pub mod gamepad;
pub mod group_menu_members;
pub mod host_hardware;
pub mod idle_lock;
pub mod login_session;
pub mod media;
pub mod memory_board;
pub mod niri_session;
pub mod notification_toasts;
pub mod nspawn_containers;
pub mod podman_engines;
pub mod shell_settings;
pub mod systemd_service_units;
pub mod tailnet;
pub mod virtual_machines;
pub mod voice_dictation;
pub mod wallpaper;
pub mod weather;
pub mod wireguard_tunnels;
