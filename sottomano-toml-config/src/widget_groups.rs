pub mod caldav_account;
pub mod gamepad_bindings;
pub mod media_lyrics;
pub mod memory_growth_trend;
pub mod rain_alerts;
pub mod reachability;
pub mod session_focus;
pub mod spend_budget;
pub mod systemd_manager;
pub mod temperature_sensors;
pub mod tray;

use serde::{Deserialize, Serialize};

use crate::{
    spacing::Spacing,
    widget_groups::{
        caldav_account::CaldavAccountConfig,
        gamepad_bindings::GamepadBindingsConfig,
        media_lyrics::MediaLyricsConfig,
        memory_growth_trend::MemoryGrowthTrendConfig,
        rain_alerts::RainAlertsConfig,
        reachability::ReachabilityConfig,
        session_focus::SessionFocusConfig,
        spend_budget::{ClaudeCodeBudgetConfig, CopilotCliBudgetConfig, OpenRouterBudgetConfig},
        systemd_manager::SystemdManagerConfig,
        temperature_sensors::SensorConfig,
        tray::TrayConfig,
    },
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WidgetGroupsConfig {
    pub window: WindowGroupConfig,
    pub workspaces: WorkspacesConfig,
    pub tray: TrayConfig,
    pub compute: ComputeGroupConfig,
    pub memory_board: MemoryBoardGroupConfig,
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
pub struct MemoryBoardGroupConfig {
    pub growth_trend: MemoryGrowthTrendConfig,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementVisibility {
    #[default]
    Shown,
    Hidden,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalAgentUse {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowGroupConfig {
    pub audio_indicator: ElementVisibility,
    pub progress: ElementVisibility,
    pub count_badge: ElementVisibility,
    pub nixpkgs_branch: String,
    pub maximum_window_title_width: Spacing,
    pub agents: WindowAgentsConfig,
}

impl WindowGroupConfig {
    #[must_use]
    pub fn resolved_maximum_window_title_width(&self, em_px: f32) -> f32 {
        self.maximum_window_title_width.resolve(em_px)
    }
}

impl Default for WindowGroupConfig {
    fn default() -> Self {
        Self {
            audio_indicator: ElementVisibility::Shown,
            progress: ElementVisibility::Shown,
            count_badge: ElementVisibility::Shown,
            nixpkgs_branch: "nixpkgs-unstable".to_owned(),
            maximum_window_title_width: Spacing::Px(200.0),
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
    pub kitty: TerminalAgentUse,
    pub wezterm: TerminalAgentUse,
    pub pty_shells: TerminalAgentUse,
}

impl Default for WindowAgentsConfig {
    fn default() -> Self {
        Self {
            kitty: TerminalAgentUse::Enabled,
            wezterm: TerminalAgentUse::Enabled,
            pty_shells: TerminalAgentUse::Enabled,
        }
    }
}

#[cfg(test)]
mod window_title_width_tests {
    use super::{ElementVisibility, TerminalAgentUse, WindowAgentsConfig, WindowGroupConfig};

    #[test]
    fn window_indicators_read_as_shown_or_hidden() {
        let window_group: WindowGroupConfig =
            toml::from_str("audio_indicator = \"hidden\"\n").unwrap();
        assert_eq!(window_group.audio_indicator, ElementVisibility::Hidden);
        assert_eq!(window_group.progress, ElementVisibility::Shown);
        assert!(toml::from_str::<WindowGroupConfig>("progress = true\n").is_err());
    }

    #[test]
    fn terminal_agents_read_as_enabled_or_disabled() {
        let agents: WindowAgentsConfig = toml::from_str("kitty = \"disabled\"\n").unwrap();
        assert_eq!(agents.kitty, TerminalAgentUse::Disabled);
        assert_eq!(agents.wezterm, TerminalAgentUse::Enabled);
    }

    #[test]
    fn maximum_window_title_width_follows_the_font_in_em() {
        let window_group: WindowGroupConfig =
            toml::from_str("maximum_window_title_width = \"10em\"\n").unwrap();
        assert!(
            (window_group.resolved_maximum_window_title_width(14.0) - 140.0).abs() < f32::EPSILON
        );
    }
}
