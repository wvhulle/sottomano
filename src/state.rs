pub mod charge_power_curve;
pub mod dark_mode;
pub mod idle;
pub mod launches;
pub mod niri_rules;
pub mod overlay_settings;
pub mod power;
pub mod rate_limit;
pub mod shell_settings;
pub mod store;

use core::f32::consts::FRAC_1_SQRT_2;

use iced::{
    Color, Shadow, Vector,
    border::{self, Radius},
    widget::container,
};
pub(crate) use store::{Columns, StoreError, connect};

use crate::{config::Config, widgets::panel_frame::OutlinedSides};

#[derive(Debug, Clone, Default)]
pub struct ShellSettings {
    pub layout: ShellLayout,
    pub panels: PanelAppearance,
    pub shadow: ShadowStyle,
    pub notifications: NotificationSettings,
    pub overlays: OverlaySettings,
}

impl ShellSettings {
    pub fn frame_style(&self, config: &Config) -> container::Style {
        let palette = &config.theme.palette;
        container::Style::default()
            .background(palette.surface.scale_alpha(self.panels.opacity))
            .border(
                border::rounded(self.panels.corner_radius)
                    .color(palette.frame_outline)
                    .width(config.panel_bar.frame_outline_px),
            )
            .shadow(self.shadow.drop_shadow())
            .color(palette.bar_foreground)
    }

    pub fn outlined_sides(&self, touching_screen_edges: OutlinedSides) -> OutlinedSides {
        if self.layout.float_offset_px > 0.0 {
            OutlinedSides::ALL
        } else {
            touching_screen_edges
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OverlaySettings {
    pub opacity: f32,
    pub corner_radius: Radius,
    pub toast_spatial_layout: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            opacity: 0.85,
            corner_radius: Radius::from(8.0),
            toast_spatial_layout: true,
        }
    }
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

impl ShadowStyle {
    pub fn drop_shadow(&self) -> Shadow {
        if !self.cast {
            return Shadow::default();
        }
        Shadow {
            color: Color::BLACK.scale_alpha(self.opacity),
            offset: Vector::ZERO,
            blur_radius: self.softness_px,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShellLayout {
    pub disconnected_groups: bool,
    pub auto_collapse_groups: bool,
    pub workspaces_placement: WorkspacesPlacement,
    pub float_offset_px: f32,
}

impl Default for ShellLayout {
    fn default() -> Self {
        Self {
            disconnected_groups: false,
            auto_collapse_groups: true,
            workspaces_placement: WorkspacesPlacement::Sidebar,
            float_offset_px: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PanelAppearance {
    pub opacity: f32,
    pub corner_radius: Radius,
}

impl Default for PanelAppearance {
    fn default() -> Self {
        Self {
            opacity: 0.8,
            corner_radius: Radius::default(),
        }
    }
}

impl PanelAppearance {
    pub fn frame_corner_inset(&self) -> f32 {
        self.corner_radius.top_left * (1.0 - FRAC_1_SQRT_2)
    }
}

#[derive(Debug, Clone)]
pub struct NotificationSettings {
    pub do_not_disturb: bool,
    pub sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            do_not_disturb: false,
            sound: true,
        }
    }
}
