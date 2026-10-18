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

use core::f32::consts::FRAC_1_SQRT_2;

use iced::{Border, Color, Shadow, Vector, border::Radius, widget::container};
use vetrata_config::Config;

use crate::widgets::panel_frame::OutlinedSides;

#[derive(Debug, Clone)]
pub struct ShellSettings {
    pub layout: ShellLayout,
    pub panels: LayerAppearance,
    pub sidebar: LayerAppearance,
    pub overlays: LayerAppearance,
    pub notifications: NotificationSettings,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            layout: ShellLayout::default(),
            panels: LayerAppearance::default(),
            sidebar: LayerAppearance::default(),
            overlays: LayerAppearance {
                opacity: 0.85,
                ..LayerAppearance::default()
            },
            notifications: NotificationSettings::default(),
        }
    }
}

impl ShellSettings {
    #[must_use]
    pub fn outlined_sides(&self, touching_screen_edges: OutlinedSides) -> OutlinedSides {
        if self.layout.float_offset_px > 0.0 {
            OutlinedSides::ALL
        } else {
            touching_screen_edges
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLayer {
    Panels,
    Sidebar,
    Overlays,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerAppearance {
    pub opacity: f32,
    pub corner_radius: Radius,
    pub blurred: bool,
    pub shadow: ShadowStyle,
    pub outline: OutlineStyle,
}

impl Default for LayerAppearance {
    fn default() -> Self {
        Self {
            opacity: 0.8,
            corner_radius: Radius::from(8.0),
            blurred: false,
            shadow: ShadowStyle::default(),
            outline: OutlineStyle::default(),
        }
    }
}

impl LayerAppearance {
    #[must_use]
    pub fn frame_style(&self, config: &Config) -> container::Style {
        let palette = &config.theme.palette;
        container::Style::default()
            .background(palette.surface.scale_alpha(self.opacity))
            .border(Border {
                radius: self.corner_radius,
                ..self.outline.border(config)
            })
            .shadow(self.shadow.drop_shadow())
            .color(palette.bar_foreground)
    }

    #[must_use]
    pub fn frame_corner_inset(&self) -> f32 {
        self.corner_radius.top_left * (1.0 - FRAC_1_SQRT_2)
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

impl ShadowStyle {
    #[must_use]
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineStyle {
    pub drawn: bool,
}

impl Default for OutlineStyle {
    fn default() -> Self {
        Self { drawn: true }
    }
}

impl OutlineStyle {
    #[must_use]
    pub fn border(self, config: &Config) -> Border {
        if !self.drawn {
            return Border::default();
        }
        Border {
            color: config.theme.palette.frame_outline,
            width: config.panel_bar.frame_outline,
            radius: Radius::default(),
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
