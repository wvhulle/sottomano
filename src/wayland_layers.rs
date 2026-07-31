//! Everything that owns or describes a Wayland layer-shell surface: the bar
//! strips and their shadow backdrop ([`surface`]), and the hosts that open
//! their own surfaces on top of the bar — hover [`tooltip`]s, the tray context
//! [`tray_menu`], and [`notifications`] toasts.

pub mod backdrop_shadow;
pub mod bar;
pub mod blur_caster;
pub mod notifications;
pub mod surface;
pub mod tooltip;
pub mod tray_menu;
