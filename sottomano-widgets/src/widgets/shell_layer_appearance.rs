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
                corner_radius_px: 12.0,
                shadow: ShadowStyle {
                    cast: true,
                    softness_px: 16.0,
                    opacity: 0.18,
                },
                outline: OutlineStyle {
                    drawn: true,
                    opacity: 1.0,
                },
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
    pub opacity: f32,
}

impl Default for OutlineStyle {
    fn default() -> Self {
        Self {
            drawn: true,
            opacity: 1.0,
        }
    }
}
