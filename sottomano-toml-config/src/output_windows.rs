use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{element_visibility::ElementVisibility, spacing::Spacing};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalAgentUse {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputWindowsConfig {
    pub audio_indicator: ElementVisibility,
    pub progress: ElementVisibility,
    pub count_badge: ElementVisibility,
    pub nixpkgs_branch: String,
    pub nixos_flake: PathBuf,
    pub maximum_window_title_width: Spacing,
    #[serde(with = "crate::friendly_duration")]
    pub workspace_slide_duration: Duration,
    pub agents: WindowAgentsConfig,
}

impl Default for OutputWindowsConfig {
    fn default() -> Self {
        Self {
            audio_indicator: ElementVisibility::Shown,
            progress: ElementVisibility::Shown,
            count_badge: ElementVisibility::Shown,
            nixpkgs_branch: "nixpkgs-unstable".to_owned(),
            nixos_flake: PathBuf::from("/etc/nixos"),
            maximum_window_title_width: Spacing::Px(200.0),
            workspace_slide_duration: Duration::from_millis(250),
            agents: WindowAgentsConfig::default(),
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
    use super::{ElementVisibility, OutputWindowsConfig, TerminalAgentUse, WindowAgentsConfig};

    #[test]
    fn window_indicators_read_as_shown_or_hidden() {
        let window_group: OutputWindowsConfig =
            toml::from_str("audio_indicator = \"hidden\"\n").unwrap();
        assert_eq!(window_group.audio_indicator, ElementVisibility::Hidden);
        assert_eq!(window_group.progress, ElementVisibility::Shown);
        assert!(toml::from_str::<OutputWindowsConfig>("progress = true\n").is_err());
    }

    #[test]
    fn terminal_agents_read_as_enabled_or_disabled() {
        let agents: WindowAgentsConfig = toml::from_str("kitty = \"disabled\"\n").unwrap();
        assert_eq!(agents.kitty, TerminalAgentUse::Disabled);
        assert_eq!(agents.wezterm, TerminalAgentUse::Enabled);
    }

    #[test]
    fn maximum_window_title_width_follows_the_font_in_em() {
        let window_group: OutputWindowsConfig =
            toml::from_str("maximum_window_title_width = \"10em\"\n").unwrap();
        assert!(
            (window_group.maximum_window_title_width.resolve(14.0) - 140.0).abs() < f32::EPSILON
        );
    }
}
