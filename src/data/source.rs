use std::{
    fmt::{Debug, Display},
    future::Future,
    hash::Hash,
    time::Duration,
};

pub mod ai;
pub mod app_icon;
pub mod app_rules;
pub mod audio;
pub mod backlight;
pub mod bus;
pub mod clight;
pub mod command;
pub mod coredumps;
pub mod desktop_entries;
pub mod disk;
pub mod favicons;
pub mod fwupd;
pub mod gamma;
pub mod gpu;
pub mod hardware;
pub mod http;
pub mod icon_trim;
pub mod job_view;
pub mod journal;
pub mod kernel_errors;
pub mod launch_tracker;
pub mod launcher_entry;
pub mod logind;
pub mod mpris;
pub mod net_interfaces;
pub mod niri;
pub mod nix_store;
pub mod oom_kills;
pub mod power_profile;
pub mod previous_shutdown;
pub mod process;
pub mod screen_temperature;
pub mod service_errors;
pub mod sysfs_sensors;
pub mod systemd_manager;
pub mod tailscale;
pub mod throughput;
pub mod tray;
pub mod umbra;
pub mod wallpaper;
pub mod weather;
pub mod window_agents;
pub mod wireguard;

pub trait Source: Clone + PartialEq + Sized + Send + 'static {
    type Error: Clone + Debug + Display + Send + 'static;
    type Key: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
    const INTERVAL: Duration;

    fn available(key: &Self::Key) -> impl Future<Output = bool> + Send {
        let _ = key;
        async { true }
    }

    fn poll(key: Self::Key) -> impl Future<Output = Result<Self, Self::Error>> + Send;
}
