pub mod audio_group;
pub mod claude_usage_group;
pub mod clock_group;
pub mod color_temperature_group;
pub mod compute_group;
pub mod copilot_usage_group;
pub mod dark_mode_group;
pub mod disk_group;
pub mod gpu_group;
pub mod idle_timeouts_group;
pub mod internet_group;
pub mod journal_events_group;
pub mod llama_server_group;
pub mod login_session_group;
pub mod media_group;
pub mod monitor_group;
pub mod net_interfaces_group;
pub mod niri_window_group;
pub mod notification_group;
pub mod openrouter_credit_group;
pub mod overlay_settings_group;
pub mod panel_settings_group;
pub mod pending_upgrades_group;
pub mod power_group;
pub mod quiet_schedule_group;
pub mod session_focus_group;
pub mod session_menu;
pub mod sidebar_settings_group;
pub mod systemd_manager_group;
pub mod terminal_command;
pub mod tray_group;
pub mod wallpaper_group;
pub mod weather_group;
pub mod window_group;
pub mod workspaces_group;

use vetrata_sources::{bus, command, http, sysfs_sensors};

use crate::widgets::tooltip_body::{KeyValue, Section};

impl From<bus::Error> for Section {
    fn from(error: bus::Error) -> Self {
        Section::error(error)
    }
}

impl From<zbus::Error> for Section {
    fn from(error: zbus::Error) -> Self {
        Section::error(error)
    }
}

impl From<sysfs_sensors::Error> for Section {
    fn from(error: sysfs_sensors::Error) -> Self {
        Section::error(error)
    }
}

impl From<command::Error> for Section {
    fn from(error: command::Error) -> Self {
        Section::error(error)
    }
}

impl From<http::Error> for Section {
    fn from(error: http::Error) -> Self {
        match error {
            http::Error::Status { status, message } => Section::dictionary(
                message
                    .map(|message| KeyValue::new("Error", message))
                    .into_iter()
                    .chain([KeyValue::new("Status", status.to_string())]),
            ),
            other => Section::error(other),
        }
    }
}
