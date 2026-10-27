pub mod charge_power_curve;
pub mod dark_mode;
pub mod focus_spans;
pub mod focus_tracker;
pub mod idle;
pub mod launches;
pub mod layer_appearance;
pub mod niri_rules;
pub mod power;
pub mod quiet_hours;
pub mod shell_settings;
pub mod switch_heat_borders;
pub mod working_days;

use core::f32::consts::FRAC_1_SQRT_2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLayer {
    Panels,
    Sidebar,
    Overlays,
}

impl ShellLayer {
    #[must_use]
    pub fn default_appearance(self) -> LayerAppearance {
        match self {
            Self::Panels | Self::Sidebar => LayerAppearance::default(),
            Self::Overlays => LayerAppearance {
                opacity: 0.85,
                ..LayerAppearance::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerAppearance {
    pub opacity: f32,
    pub corner_radius_px: f32,
    pub blurred: bool,
    pub shadow: ShadowStyle,
    pub outline: OutlineStyle,
}

impl Default for LayerAppearance {
    fn default() -> Self {
        Self {
            opacity: 0.8,
            corner_radius_px: 8.0,
            blurred: true,
            shadow: ShadowStyle::default(),
            outline: OutlineStyle::default(),
        }
    }
}

impl LayerAppearance {
    #[must_use]
    pub fn frame_corner_inset(&self) -> f32 {
        self.corner_radius_px * (1.0 - FRAC_1_SQRT_2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacesPlacement {
    Sidebar,
    BottomPanel,
}

impl WorkspacesPlacement {
    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Sidebar => Self::BottomPanel,
            Self::BottomPanel => Self::Sidebar,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowStyle {
    pub cast: bool,
    pub softness_px: f32,
    pub opacity: f32,
}

impl Default for ShadowStyle {
    fn default() -> Self {
        Self {
            cast: true,
            softness_px: 20.0,
            opacity: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineStyle {
    pub drawn: bool,
}

impl Default for OutlineStyle {
    fn default() -> Self {
        Self { drawn: true }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NotificationSettings {
    pub sound: bool,
    pub toast_spatial_layout: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            sound: true,
            toast_spatial_layout: true,
        }
    }
}
