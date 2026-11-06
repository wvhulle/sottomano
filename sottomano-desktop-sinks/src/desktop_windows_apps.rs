pub mod clipboard_copy;
pub mod compositor_requests;
#[cfg(feature = "hyprland")]
pub mod hyprland_dispatch_requests;
#[cfg(feature = "niri")]
pub mod niri_action_requests;
pub mod status_notifier_tray_activation;
#[cfg(feature = "sway")]
pub mod sway_command_requests;
#[cfg(any(feature = "sway", feature = "hyprland"))]
pub mod systemd_run_launches;
