use core::f32::consts::FRAC_1_SQRT_2;

pub use corredo::frame_style::{FrameBackground, FrameStyle, OutlineStyle, ShadowStyle};

const LOOK_MATCH_TOLERANCE: f32 = 0.005;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLayer {
    Panels,
    Sidebar,
    Overlays,
    Menus,
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
            Self::Menus => LayerAppearance {
                background: FrameBackground::Weak,
                ..Self::Overlays.default_appearance()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerAppearance {
    pub background: FrameBackground,
    pub opacity: f32,
    pub corner_radius_px: f32,
    pub blurred: bool,
    pub shadow: ShadowStyle,
    pub outline: OutlineStyle,
}

impl Default for LayerAppearance {
    fn default() -> Self {
        Self {
            background: FrameBackground::Base,
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
    pub fn frame_style(&self) -> FrameStyle {
        FrameStyle {
            background: self.background,
            opacity: self.opacity,
            corner_radius_px: self.corner_radius_px,
            shadow: self.shadow,
            outline: self.outline,
            ..FrameStyle::default()
        }
    }

    #[must_use]
    pub fn with_background_faded(self, fade_level: f32) -> Self {
        let remaining = 1.0 - fade_level;
        Self {
            opacity: self.opacity * remaining,
            blurred: self.blurred && fade_level <= 0.0,
            shadow: ShadowStyle {
                opacity: self.shadow.opacity * remaining,
                ..self.shadow
            },
            ..self
        }
    }

    #[must_use]
    pub fn frame_corner_inset(&self) -> f32 {
        self.corner_radius_px * (1.0 - FRAC_1_SQRT_2)
    }

    #[must_use]
    pub fn looks_like(&self, other: &Self) -> bool {
        let close = |current: f32, wanted: f32| (current - wanted).abs() < LOOK_MATCH_TOLERANCE;
        close(self.opacity, other.opacity)
            && close(self.corner_radius_px, other.corner_radius_px)
            && self.blurred == other.blurred
            && self.shadow.cast == other.shadow.cast
            && close(self.shadow.softness_px, other.shadow.softness_px)
            && close(self.shadow.opacity, other.shadow.opacity)
            && self.outline.drawn == other.outline.drawn
            && close(self.outline.opacity, other.outline.opacity)
    }
}

#[cfg(test)]
mod tests {
    use super::ShellLayer;

    #[test]
    fn a_fully_faded_background_keeps_only_its_outline() {
        let resting = ShellLayer::Overlays.default_appearance();
        let faded = resting.with_background_faded(1.0);
        assert!(faded.opacity <= 0.0);
        assert!(faded.shadow.opacity <= 0.0);
        assert!(!faded.blurred);
        assert_eq!(faded.outline, resting.outline);
    }

    #[test]
    fn a_resting_background_keeps_its_appearance() {
        let resting = ShellLayer::Overlays.default_appearance();
        assert_eq!(resting.with_background_faded(0.0), resting);
    }

    #[test]
    fn blur_drops_as_soon_as_fading_starts() {
        let resting = ShellLayer::Overlays.default_appearance();
        assert!(!resting.with_background_faded(0.01).blurred);
    }
}
