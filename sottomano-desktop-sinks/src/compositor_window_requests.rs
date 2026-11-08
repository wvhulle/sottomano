pub mod compositor_requests;
pub mod compositor_rule_files;
#[cfg(feature = "hyprland")]
pub mod hyprland_dispatch_requests;
#[cfg(feature = "hyprland")]
pub mod hyprland_lua_rules;
#[cfg(feature = "niri")]
pub mod niri_action_requests;
#[cfg(feature = "niri")]
pub mod niri_kdl_rules;
#[cfg(feature = "sway")]
pub mod sway_command_requests;
#[cfg(feature = "sway")]
pub mod sway_include_rules;
#[cfg(any(feature = "sway", feature = "hyprland"))]
pub mod systemd_run_launches;
