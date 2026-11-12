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
        .expect("showcase clocks advance from the showcase epoch");
    u32::try_from(elapsed.as_millis() / LIVE_INTERVAL.as_millis())
        .expect("a showcase runs for fewer than u32::MAX live intervals")
}

pub fn cycled<T: Copy>(values: &[T], tick: u32) -> T {
    values[usize::try_from(tick).expect("usize holds a u32 on supported targets") % values.len()]
}

pub fn unchanging(_clock: Clock) -> Vec<ShowcaseMessage> {
    Vec::new()
}
pub mod app_launcher;
pub mod appearance_presets;
pub mod apptainer_containers;
pub mod audio;
pub mod caldav;
pub mod cgroup_memory_limits;
pub mod ci_runner_tasks;
pub mod claude_usage;
pub mod compute;
pub mod desktop_session;
pub mod docker_engines;
pub mod flatpak_sandboxes;
pub mod gamepad;
pub mod group_menu_members;
pub mod host_hardware;
pub mod idle_lock;
pub mod incus_instances;
pub mod login_session;
pub mod lxc_containers;
pub mod media;
pub mod memory_board;
pub mod microvm_guests;
pub mod notification_toasts;
pub mod nspawn_containers;
pub mod podman_engines;
pub mod shell_settings;
pub mod systemd_service_units;
pub mod tailnet;
pub mod virtual_machines;
pub mod virtualbox_machines;
pub mod vmware_machines;
pub mod voice_dictation;
pub mod wallpaper;
pub mod waydroid_container;
pub mod weather;
pub mod wireguard_tunnels;
