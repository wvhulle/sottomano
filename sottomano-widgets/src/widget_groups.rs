pub mod app_launcher_group;
pub mod appearance_presets_group;
pub mod audio_group;
pub mod bluetooth_adapters_group;
pub mod caldav_group;
pub mod claude_usage_group;
pub mod clock_group;
pub mod color_temperature_group;
pub mod compute_group;
pub mod copilot_usage_group;
pub mod dark_mode_group;
mod echo_probe_window_rows;
pub mod ethernet_ports_group;
pub mod expand_animation;
pub mod fixed_panel_group;
pub mod gamepad_group;
pub mod gpu_group;
pub mod idle_timeouts_group;
mod interface_address_sections;
mod interface_group_titles;
pub mod journal_events_group;
pub mod llama_server_group;
pub mod login_session_group;
pub mod media_group;
pub mod memory_board_group;
pub mod mobile_modems_group;
pub mod monitor_group;
pub mod niri_window_group;
pub mod notification_group;
pub mod openrouter_credit_group;
pub mod overlay_settings_group;
pub mod panel_settings_group;
pub mod pending_upgrades_group;
pub mod physical_drive_group;
pub mod power_group;
pub mod quiet_schedule_group;
pub mod reachability_group;
pub mod session_focus_group;
pub mod session_power_group;
pub mod sidebar_settings_group;
mod sqlite_singleton_flag;
pub mod storage_space_group;
pub mod systemd_manager_group;
pub mod terminal_command;
pub mod tray_group;
pub mod voice_dictation_group;
pub mod vpn_group;
pub mod wallpaper_group;
pub mod weather_group;
pub mod widget_group_content;
pub mod widget_group_identity;
pub mod wifi_adapters_group;
pub mod window_group;
pub mod workspaces_group;

use sottomano_sources::{
    ai_usage_spend::gguf_header_model_card::GgufHeaderError,
    system_protocol_clients::{
        dbus_desktop_buses, http_json_client, shell_command_output, sysfs_attribute_reads,
    },
};

use crate::{hover_cards::hover_card_body::Section, widgets::value_cells::LabeledValue};

impl From<GgufHeaderError> for Section {
    fn from(error: GgufHeaderError) -> Self {
        Section::error(error)
    }
}

impl From<dbus_desktop_buses::Error> for Section {
    fn from(error: dbus_desktop_buses::Error) -> Self {
        Section::error(error)
    }
}

impl From<zbus::Error> for Section {
    fn from(error: zbus::Error) -> Self {
        Section::error(error)
    }
}

impl From<sysfs_attribute_reads::Error> for Section {
    fn from(error: sysfs_attribute_reads::Error) -> Self {
        Section::error(error)
    }
}

impl From<shell_command_output::Error> for Section {
    fn from(error: shell_command_output::Error) -> Self {
        Section::error(error)
    }
}

impl From<http_json_client::Error> for Section {
    fn from(error: http_json_client::Error) -> Self {
        match error {
            http_json_client::Error::Status { status, message } => Section::dictionary(
                message
                    .map(|message| LabeledValue::new("Error", message))
                    .into_iter()
                    .chain([LabeledValue::new("Status", status.to_string())]),
            ),
            other => Section::error(other),
        }
    }
}
