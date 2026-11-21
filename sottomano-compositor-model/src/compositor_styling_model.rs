use std::f32::consts::SQRT_2;

use sottomano_toml_config::screen_edges::Position;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppWindowLayout {
    pub window_corner_radius_px: f32,
    pub gaps_px: f32,
}

impl Default for AppWindowLayout {
    fn default() -> Self {
        Self {
            window_corner_radius_px: 0.0,
            gaps_px: 16.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ShellBlurDepth {
    #[default]
    WindowsBehind,
    WallpaperOnly,
}

impl ShellBlurDepth {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ShellBlurDepth::WindowsBehind => ShellBlurDepth::WallpaperOnly,
            ShellBlurDepth::WallpaperOnly => ShellBlurDepth::WindowsBehind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CompositorStyling {
    pub blur_depth: ShellBlurDepth,
    pub layout: AppWindowLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    NotificationToasts,
    Backdrop(Position),
}

impl StyledSurface {
    pub const ALL: [Self; 4] = [
        Self::Panel,
        Self::NotificationToasts,
        Self::Backdrop(Position::Top),
        Self::Backdrop(Position::Bottom),
    ];

    #[must_use]
    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "sottomano-panel",
            Self::NotificationToasts => "sottomano-notification-toasts",
            Self::Backdrop(Position::Top) => "sottomano-backdrop-top",
            Self::Backdrop(Position::Bottom) => "sottomano-backdrop-bottom",
            Self::Backdrop(Position::Left) => "sottomano-backdrop-left",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatedWindow {
    pub app_id: Option<String>,
    pub title_without_status_prefix: String,
    pub border_color: String,
}

impl HeatedWindow {
    #[must_use]
    pub fn titled(app_id: Option<String>, title: &str, border_color: String) -> Option<Self> {
        let title_without_status_prefix =
            title.trim_start_matches(|character: char| !character.is_alphanumeric());
        (!title_without_status_prefix.is_empty()).then(|| Self {
            app_id,
            title_without_status_prefix: title_without_status_prefix.to_owned(),
            border_color,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SwitchHeat {
    #[default]
    Hidden,
    Shown {
        focus_ring_color: String,
        neutral_border_color: String,
        windows: Vec<HeatedWindow>,
    },
}

impl SwitchHeat {
    #[must_use]
    pub fn heated_windows(&self) -> &[HeatedWindow] {
        match self {
            Self::Hidden => &[],
            Self::Shown { windows, .. } => windows,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeShadowRendering {
    CompositorShadow,
    SelfDrawnFalloff,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeShadowGeometry {
    pub softness_px: f32,
    pub spread_px: f32,
    pub lift_px: f32,
    pub alpha: u8,
}

pub const PANEL_EDGE_SHADOW: EdgeShadowGeometry = EdgeShadowGeometry {
    softness_px: 27.0,
    spread_px: 25.0,
    lift_px: 3.0,
    alpha: 0x60,
};

const FALLOFF_SIGMAS_SHOWN: f32 = 3.0;

fn complementary_error_function(argument: f32) -> f32 {
    let magnitude = argument.abs();
    let rational_term = 1.0 / (1.0 + 0.327_591_1 * magnitude);
    let polynomial = rational_term
        * (0.254_829_6
            + rational_term
                * (-0.284_496_7
                    + rational_term
                        * (1.421_413_8
                            + rational_term * (-1.453_152 + rational_term * 1.061_405_4))));
    let tail = polynomial * (-magnitude * magnitude).exp();
    if argument >= 0.0 { tail } else { 2.0 - tail }
}

impl EdgeShadowGeometry {
    fn blur_sigma_px(self) -> f32 {
        self.softness_px / 2.0
    }

    fn solid_depth_px(self) -> f32 {
        self.spread_px - self.lift_px
    }

    #[must_use]
    pub fn reach_px(self) -> f32 {
        (self.solid_depth_px() + FALLOFF_SIGMAS_SHOWN * self.blur_sigma_px()).ceil()
    }

    #[must_use]
    pub fn opacity_at(self, distance_from_edge_px: f32) -> f32 {
        let normalized_distance =
            (distance_from_edge_px - self.solid_depth_px()) / (self.blur_sigma_px() * SQRT_2);
        f32::from(self.alpha) / 255.0 * 0.5 * complementary_error_function(normalized_distance)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylingControl {
    WindowCornerRadius,
    WindowGaps,
    ShellBlurDepth,
    SwitchHeatBorders,
}

#[must_use]
pub fn escaped_for_regex(literal: &str) -> String {
    literal
        .chars()
        .filter(|character| !character.is_control())
        .flat_map(|character| {
            let metacharacter = "\\.+*?()|[]{}^$#&-~".contains(character);
            metacharacter.then_some('\\').into_iter().chain([character])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::PANEL_EDGE_SHADOW;

    #[test]
    fn the_falloff_fades_monotonically_to_nothing_at_its_reach() {
        let reach = PANEL_EDGE_SHADOW.reach_px();
        let samples: Vec<f32> = (0..=reach as u32)
            .map(|distance| PANEL_EDGE_SHADOW.opacity_at(distance as f32))
            .collect();
        assert!(samples.windows(2).all(|pair| pair[1] <= pair[0]));
        assert!(PANEL_EDGE_SHADOW.opacity_at(reach) < 1.0 / 255.0);
    }

    #[test]
    fn the_screen_edge_is_nearly_as_dark_as_the_shadow_color() {
        let edge_opacity = PANEL_EDGE_SHADOW.opacity_at(0.0);
        let full_opacity = f32::from(PANEL_EDGE_SHADOW.alpha) / 255.0;
        assert!(edge_opacity > 0.9 * full_opacity && edge_opacity <= full_opacity);
    }
}
