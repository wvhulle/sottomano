use core::f32::consts::FRAC_1_SQRT_2;

use corredo::theming::{
    FrameBackground, FrameStyle, OutlineStyle, OutlineTracing, ShadowCasting, ShadowStyle,
};
use sottomano_toml_config::screen_edges::Position;

use crate::desktop_panels::region_reporter::BackdropBlur;

const LOOK_MATCH_TOLERANCE: f32 = 0.005;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellLayer {
    Panels,
    Sidebar,
    Overlays,
    Menus,
}

impl From<Position> for ShellLayer {
    fn from(position: Position) -> Self {
        match position {
            Position::Left => Self::Sidebar,
            Position::Top | Position::Bottom => Self::Panels,
        }
    }
}

impl ShellLayer {
    #[must_use]
    pub fn default_appearance(self) -> LayerAppearance {
        match self {
            Self::Panels => LayerAppearance::default(),
            Self::Sidebar => LayerAppearance {
                frame: FrameStyle {
                    opacity: 0.9,
                    ..FrameStyle::default()
                },
                ..LayerAppearance::default()
            },
            Self::Overlays => LayerAppearance {
                frame: FrameStyle {
                    opacity: 0.85,
                    corner_radius_px: 12.0,
                    shadow: ShadowStyle {
                        casting: ShadowCasting::Cast,
                        softness_px: 16.0,
                        opacity: 0.35,
                    },
                    outline: OutlineStyle {
                        tracing: OutlineTracing::Drawn,
                        opacity: 1.0,
                    },
                    ..FrameStyle::default()
                },
                ..LayerAppearance::default()
            },
            Self::Menus => {
                let overlays = Self::Overlays.default_appearance();
                LayerAppearance {
                    frame: FrameStyle {
                        background: FrameBackground::Weak,
                        ..overlays.frame
                    },
                    ..overlays
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerAppearance {
    pub frame: FrameStyle,
    pub backdrop_blur: BackdropBlur,
}

impl Default for LayerAppearance {
    fn default() -> Self {
        Self {
            frame: FrameStyle::default(),
            backdrop_blur: BackdropBlur::Blurred,
        }
    }
}

impl LayerAppearance {
    #[must_use]
    pub fn with_background_faded(self, fade_level: f32) -> Self {
        let remaining = 1.0 - fade_level;
        Self {
            frame: FrameStyle {
                opacity: self.frame.opacity * remaining,
                shadow: ShadowStyle {
                    opacity: self.frame.shadow.opacity * remaining,
                    ..self.frame.shadow
                },
                ..self.frame
            },
            backdrop_blur: if fade_level <= 0.0 {
                self.backdrop_blur
            } else {
                BackdropBlur::Clear
            },
        }
    }

    #[must_use]
    pub fn frame_corner_inset(&self) -> f32 {
        self.frame.corner_radius_px * (1.0 - FRAC_1_SQRT_2)
    }

    #[must_use]
    pub fn look_comparison(&self, other: &Self) -> LookComparison {
        let close = |current: f32, wanted: f32| (current - wanted).abs() < LOOK_MATCH_TOLERANCE;
        let frame = self.frame;
        let other_frame = other.frame;
        if close(frame.opacity, other_frame.opacity)
            && close(frame.corner_radius_px, other_frame.corner_radius_px)
            && self.backdrop_blur == other.backdrop_blur
            && frame.shadow.casting == other_frame.shadow.casting
            && close(frame.shadow.softness_px, other_frame.shadow.softness_px)
            && close(frame.shadow.opacity, other_frame.shadow.opacity)
            && frame.outline.tracing == other_frame.outline.tracing
            && close(frame.outline.opacity, other_frame.outline.opacity)
        {
            LookComparison::Matching
        } else {
            LookComparison::Differing
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookComparison {
    Matching,
    Differing,
}

#[cfg(test)]
mod tests {
    use super::ShellLayer;
    use crate::desktop_panels::region_reporter::BackdropBlur;

    #[test]
    fn a_fully_faded_background_keeps_only_its_outline() {
        let resting = ShellLayer::Overlays.default_appearance();
        let faded = resting.with_background_faded(1.0);
        assert!(faded.frame.opacity <= 0.0);
        assert!(faded.frame.shadow.opacity <= 0.0);
        assert_eq!(faded.backdrop_blur, BackdropBlur::Clear);
        assert_eq!(faded.frame.outline, resting.frame.outline);
    }

    #[test]
    fn a_resting_background_keeps_its_appearance() {
        let resting = ShellLayer::Overlays.default_appearance();
        assert_eq!(resting.with_background_faded(0.0), resting);
    }

    #[test]
    fn blur_drops_as_soon_as_fading_starts() {
        let resting = ShellLayer::Overlays.default_appearance();
        assert_eq!(
            resting.with_background_faded(0.01).backdrop_blur,
            BackdropBlur::Clear
        );
    }
}
