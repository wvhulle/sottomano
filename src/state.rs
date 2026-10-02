pub mod charge_power_curve;
pub mod dark_mode;
pub mod idle;
pub mod launches;
pub mod panel;
pub mod power;
pub mod rate_limit;
pub mod store;

use core::f32::consts::FRAC_1_SQRT_2;

use iced::{
    Color, Shadow, Vector,
    border::{self, Radius},
    widget::container,
};

use crate::config::Palette;
pub(crate) use store::{Columns, StoreError, open};

#[derive(Debug, Clone, Default)]
pub struct PanelSettings {
    pub layout: PanelLayout,
    pub toasts: ToastPreferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacesPlacement {
    Sidebar,
    BottomPanel,
}

impl WorkspacesPlacement {
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

#[derive(Debug, Clone)]
pub struct PanelLayout {
    pub shadow: ShadowStyle,
    pub disconnected_groups: bool,
    pub auto_collapse_groups: bool,
    pub workspaces_placement: WorkspacesPlacement,
    pub float_offset_px: f32,
    pub tint_factor: f32,
    pub corner_radius: Radius,
}

impl PanelLayout {
    pub fn frame_style(&self, palette: &Palette) -> container::Style {
        container::Style::default()
            .background(palette.surface.scale_alpha(self.tint_factor))
            .border(border::rounded(self.corner_radius))
            .shadow(self.frame_shadow())
            .color(palette.bar_foreground)
    }

    pub fn frame_shadow(&self) -> Shadow {
        if !self.shadow.cast {
            return Shadow::default();
        }
        Shadow {
            color: Color::BLACK.scale_alpha(self.shadow.opacity),
            offset: Vector::ZERO,
            blur_radius: self.shadow.softness_px,
        }
    }

    pub fn frame_corner_inset(&self) -> f32 {
        self.corner_radius.top_left * (1.0 - FRAC_1_SQRT_2)
    }
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            shadow: ShadowStyle::default(),
            disconnected_groups: false,
            auto_collapse_groups: true,
            workspaces_placement: WorkspacesPlacement::Sidebar,
            float_offset_px: 0.0,
            tint_factor: 0.0,
            corner_radius: Radius::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToastPreferences {
    pub do_not_disturb: bool,
    pub sound: bool,
    pub spatial_layout: bool,
}

impl Default for ToastPreferences {
    fn default() -> Self {
        Self {
            do_not_disturb: false,
            sound: true,
            spatial_layout: true,
        }
    }
}
