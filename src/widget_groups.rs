//! The stateful bar segments ("groups"): each owns its own state, `Message`,
//! `update`, and `view`, and is composed into the bar by
//! [`wayland_layers::bar`](crate::wayland_layers::bar).

pub mod audio_group;
pub mod battery_group;
pub mod clock_group;
pub mod idle_timeouts_group;
pub mod internet_group;
pub mod journal_group;
pub mod media_group;
pub mod monitor_group;
pub mod net_interfaces_group;
pub mod panel_settings_group;
pub mod scroll;
pub mod source_cell;
pub mod system_group;
pub mod tray_group;
pub mod weather_group;
pub mod window_group;
pub mod workspaces_group;
