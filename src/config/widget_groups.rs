pub mod ai_usage;
pub mod systemd_manager;
pub mod temperature_sensors;
pub mod tray;

use serde::{Deserialize, Serialize};

use crate::config::widget_groups::{
    ai_usage::AiUsageConfig, systemd_manager::SystemdManagerConfig,
    temperature_sensors::TemperatureSensorsConfig, tray::TrayConfig,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WidgetGroupsConfig {
    pub window_list: WindowListConfig,
    pub workspaces: WorkspacesConfig,
    pub tray: TrayConfig,
    pub temperature_sensors: TemperatureSensorsConfig,
    pub systemd_manager: SystemdManagerConfig,
    pub ai_usage: AiUsageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowListConfig {
    pub audio_indicator: bool,
    pub progress: bool,
    pub count_badge: bool,
    pub nixpkgs_branch: String,
    pub agents: WindowAgentsConfig,
}

impl Default for WindowListConfig {
    fn default() -> Self {
        Self {
            audio_indicator: true,
            progress: true,
            count_badge: true,
            nixpkgs_branch: "nixpkgs-unstable".to_owned(),
            agents: WindowAgentsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkspacesConfig {
    pub preview_scale: f32,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        Self {
            preview_scale: 0.19,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowAgentsConfig {
    pub kitty: bool,
}

impl Default for WindowAgentsConfig {
    fn default() -> Self {
        Self { kitty: true }
    }
}
