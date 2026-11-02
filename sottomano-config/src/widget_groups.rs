pub mod caldav_account;
pub mod gamepad_bindings;
pub mod media_lyrics;
pub mod rain_alerts;
pub mod reachability;
pub mod session_focus;
pub mod spend_budget;
pub mod systemd_manager;
pub mod temperature_sensors;
pub mod tray;

use serde::{Deserialize, Serialize};

use crate::widget_groups::{
    caldav_account::CaldavAccountConfig,
    gamepad_bindings::GamepadBindingsConfig,
    media_lyrics::MediaLyricsConfig,
    rain_alerts::RainAlertsConfig,
    reachability::ReachabilityConfig,
    session_focus::SessionFocusConfig,
    spend_budget::{ClaudeCodeBudgetConfig, CopilotCliBudgetConfig, OpenRouterBudgetConfig},
    systemd_manager::SystemdManagerConfig,
    temperature_sensors::SensorConfig,
    tray::TrayConfig,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WidgetGroupsConfig {
    pub window: WindowGroupConfig,
    pub workspaces: WorkspacesConfig,
    pub tray: TrayConfig,
    pub compute: ComputeGroupConfig,
    pub physical_drive: PhysicalDriveGroupConfig,
    pub systemd_manager: SystemdManagerConfig,
    pub claude_usage: ClaudeCodeBudgetConfig,
    pub copilot_usage: CopilotCliBudgetConfig,
    pub openrouter_credit: OpenRouterBudgetConfig,
    pub session_focus: SessionFocusConfig,
    pub weather: WeatherGroupConfig,
    pub reachability: ReachabilityConfig,
    pub gamepad: GamepadBindingsConfig,
    pub caldav: CaldavAccountConfig,
    pub media: MediaGroupConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComputeGroupConfig {
    pub temperature_sensors: SensorConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhysicalDriveGroupConfig {
    pub temperature_sensors: SensorConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherGroupConfig {
    pub rain_alerts: RainAlertsConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaGroupConfig {
    pub lyrics: MediaLyricsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowGroupConfig {
    pub audio_indicator: bool,
    pub progress: bool,
    pub count_badge: bool,
    pub nixpkgs_branch: String,
    pub agents: WindowAgentsConfig,
}

impl Default for WindowGroupConfig {
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
