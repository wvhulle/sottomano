pub mod app_launcher_group;
pub mod appearance_presets_group;
pub mod audio_group;
pub mod bluetooth_adapters_group;
pub mod caldav_group;
pub mod cgroup_memory_limits_group;
pub mod ci_runs_group;
pub mod claude_usage_group;
pub mod clock_group;
pub mod color_temperature_group;
pub mod compositor_styling_group;
pub mod compute_group;
pub mod copilot_usage_group;
pub mod dark_mode_group;
mod echo_probe_window_rows;
pub mod ethernet_ports_group;
pub mod fixed_panel_group;
pub mod gamepad_group;
pub mod global_menu_group;
pub mod gpu_group;
pub mod idle_pause_groups;
mod interface_address_sections;
mod interface_group_titles;
pub mod journal_events_group;
pub mod llama_server_group;
pub mod login_session_group;
pub mod media_group;
pub mod memory_board_group;
pub mod mobile_modems_group;
pub mod monitor_group;
pub mod network_manager_vpn_group;
pub mod notification_group;
pub mod nspawn_containers_group;
pub mod openrouter_credit_group;
pub mod overlay_settings_group;
pub mod panel_settings_group;
pub mod pending_upgrades_group;
pub mod physical_drive_group;
pub mod podman_engines_group;
pub mod power_group;
pub(crate) mod process_usage_cells;
pub mod quiet_schedule_group;
pub mod reachability_group;
pub mod session_focus_group;
pub mod session_power_group;
pub mod sidebar_settings_group;
mod sqlite_singleton_flag;
pub mod storage_space_group;
pub mod systemd_manager_group;
pub mod tailnet_group;
pub mod terminal_command;
pub mod tray_group;
pub mod virtual_machines_group;
pub mod voice_dictation_group;
pub mod wallpaper_group;
pub mod weather_group;
pub mod widget_group_content;
pub mod widget_group_identity;
pub mod wifi_adapters_group;
pub mod window_group;
pub mod wireguard_tunnels_group;
pub mod workspaces_group;

use sottomano_desktop_sources::{
    ai_usage_spend::gguf_header_model_card::GgufHeaderError,
    system_protocol_clients::{
        dbus_desktop_buses, http_json_client, shell_command_output, sysfs_attribute_reads,
    },
};

use crate::{
    hover_cards::hover_card_body::{Section, ShownAsErrorSection},
    widgets::value_cells::LabeledValue,
};

impl ShownAsErrorSection for GgufHeaderError {}

impl ShownAsErrorSection for dbus_desktop_buses::Error {}

impl ShownAsErrorSection for zbus::Error {}

impl ShownAsErrorSection for sysfs_attribute_reads::Error {}

impl ShownAsErrorSection for shell_command_output::Error {}

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
