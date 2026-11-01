use sottomano_sources::screen_brightness_color::solar_switch_schedule::ColorMode;

use crate::widgets::{
    phosphor::{Phosphor, Weight},
    shell_layer_appearance::{LayerAppearance, OutlineStyle, ShadowStyle, ShellLayer},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellAppearance {
    pub scheme: ColorMode,
    pub panels: LayerAppearance,
    pub sidebar: LayerAppearance,
    pub overlays: LayerAppearance,
}

impl Default for ShellAppearance {
    fn default() -> Self {
        Self {
            scheme: ColorMode::Dark,
            panels: ShellLayer::Panels.default_appearance(),
            sidebar: ShellLayer::Sidebar.default_appearance(),
            overlays: ShellLayer::Overlays.default_appearance(),
        }
    }
}

impl ShellAppearance {
    #[must_use]
    pub fn looks_like(&self, other: &Self) -> bool {
        self.scheme == other.scheme
            && self.panels.looks_like(&other.panels)
            && self.sidebar.looks_like(&other.sidebar)
            && self.overlays.looks_like(&other.overlays)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearancePreset {
    ClearDark,
    BrightOffice,
    FrostedDark,
}

impl AppearancePreset {
    pub const ALL: [Self; 3] = [Self::ClearDark, Self::FrostedDark, Self::BrightOffice];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ClearDark => "Clear dark",
            Self::BrightOffice => "Bright office",
            Self::FrostedDark => "Frosted dark",
        }
    }

    #[must_use]
    pub fn icon(self) -> Phosphor {
        match self {
            Self::ClearDark => Phosphor::Moon(Weight::Regular),
            Self::BrightOffice => Phosphor::Buildings(Weight::Regular),
            Self::FrostedDark => Phosphor::Snowflake(Weight::Regular),
        }
    }

    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::ClearDark => {
                "dark colours, see-through panels without outline, blur or shadow; a frosted, \
                 outlined sidebar"
            }
            Self::BrightOffice => {
                "light colours, nearly opaque panels without blur, blurred tooltips"
            }
            Self::FrostedDark => "dark colours, translucent panels frosted with blur",
        }
    }

    #[must_use]
    pub fn applied_to(self, current: ShellAppearance) -> ShellAppearance {
        let shaped = self.shaped_layers(current);
        ShellAppearance {
            overlays: LayerAppearance {
                blurred: true,
                ..shaped.overlays
            },
            ..shaped
        }
    }

    fn shaped_layers(self, current: ShellAppearance) -> ShellAppearance {
        match self {
            Self::ClearDark => {
                let cleared = |layer: LayerAppearance| LayerAppearance {
                    opacity: 0.0,
                    shadow: ShadowStyle {
                        cast: false,
                        ..layer.shadow
                    },
                    ..layer
                };
                let panels = cleared(current.panels);
                let sidebar = cleared(current.sidebar);
                ShellAppearance {
                    scheme: ColorMode::Dark,
                    panels: LayerAppearance {
                        blurred: false,
                        outline: OutlineStyle {
                            drawn: false,
                            ..panels.outline
                        },
                        ..panels
                    },
                    sidebar: LayerAppearance {
                        opacity: 0.4,
                        blurred: true,
                        outline: OutlineStyle {
                            drawn: true,
                            ..sidebar.outline
                        },
                        ..sidebar
                    },
                    ..current
                }
            }
            Self::BrightOffice => {
                let solid = |layer: LayerAppearance| LayerAppearance {
                    opacity: 0.95,
                    blurred: false,
                    ..layer
                };
                ShellAppearance {
                    scheme: ColorMode::Light,
                    panels: solid(current.panels),
                    sidebar: solid(current.sidebar),
                    overlays: solid(current.overlays),
                }
            }
            Self::FrostedDark => {
                let frosted = |layer: LayerAppearance, opacity: f32| LayerAppearance {
                    opacity,
                    blurred: true,
                    ..layer
                };
                ShellAppearance {
                    scheme: ColorMode::Dark,
                    panels: frosted(current.panels, 0.8),
                    sidebar: frosted(current.sidebar, 0.8),
                    overlays: frosted(current.overlays, 0.85),
                }
            }
        }
    }

    #[must_use]
    pub fn is_shown_by(self, current: &ShellAppearance) -> bool {
        self.applied_to(*current).looks_like(current)
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::screen_brightness_color::solar_switch_schedule::ColorMode;

    use super::{AppearancePreset, ShellAppearance};
    use crate::widgets::shell_layer_appearance::LayerAppearance;

    #[test]
    fn clear_dark_hides_the_panel_frames_but_keeps_a_frosted_outlined_sidebar() {
        let cleared = AppearancePreset::ClearDark.applied_to(ShellAppearance::default());
        assert!(cleared.panels.opacity.abs() < f32::EPSILON);
        assert!(!cleared.panels.blurred);
        assert!(!cleared.panels.outline.drawn);
        assert!(cleared.sidebar.blurred);
        assert!(cleared.sidebar.opacity > cleared.panels.opacity);
        assert!(cleared.sidebar.outline.drawn);
        assert_eq!(cleared.overlays, ShellAppearance::default().overlays);
    }

    #[test]
    fn bright_office_switches_to_light_and_near_opaque_layers() {
        let office = AppearancePreset::BrightOffice.applied_to(ShellAppearance::default());
        assert_eq!(office.scheme, ColorMode::Light);
        assert!(
            [office.panels, office.sidebar]
                .iter()
                .all(|layer| layer.opacity > 0.9 && !layer.blurred)
        );
        assert!(office.overlays.opacity > 0.9);
    }

    #[test]
    fn every_preset_blurs_behind_tooltips() {
        let unblurred = ShellAppearance {
            overlays: LayerAppearance {
                blurred: false,
                ..ShellAppearance::default().overlays
            },
            ..ShellAppearance::default()
        };
        for preset in AppearancePreset::ALL {
            assert!(preset.applied_to(unblurred).overlays.blurred, "{preset:?}");
        }
    }

    #[test]
    fn every_preset_is_shown_once_applied() {
        for preset in AppearancePreset::ALL {
            let applied = preset.applied_to(ShellAppearance::default());
            assert!(preset.is_shown_by(&applied), "{preset:?}");
        }
    }

    #[test]
    fn only_the_applied_preset_is_shown() {
        let office = AppearancePreset::BrightOffice.applied_to(ShellAppearance::default());
        assert!(!AppearancePreset::ClearDark.is_shown_by(&office));
        assert!(!AppearancePreset::FrostedDark.is_shown_by(&office));
    }
}
