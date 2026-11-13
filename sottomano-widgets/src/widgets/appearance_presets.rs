use sottomano_desktop_sources::screen_brightness_color::solar_switch_schedule::ColorMode;

use crate::{
    desktop_panels::region_reporter::BackdropBlur,
    widgets::{
        phosphor::{Phosphor, Weight},
        shell_layer_appearance::{
            LayerAppearance, LookComparison, OutlineStyle, OutlineTracing, ShadowCasting,
            ShadowStyle, ShellLayer,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellAppearance {
    pub scheme: ColorMode,
    pub panels: LayerAppearance,
    pub sidebar: LayerAppearance,
    pub overlays: LayerAppearance,
    pub menus: LayerAppearance,
}

impl Default for ShellAppearance {
    fn default() -> Self {
        Self {
            scheme: ColorMode::Dark,
            panels: ShellLayer::Panels.default_appearance(),
            sidebar: ShellLayer::Sidebar.default_appearance(),
            overlays: ShellLayer::Overlays.default_appearance(),
            menus: ShellLayer::Menus.default_appearance(),
        }
    }
}

impl ShellAppearance {
    #[must_use]
    pub fn look_comparison(&self, other: &Self) -> LookComparison {
        let layer_comparisons = [
            self.panels.look_comparison(&other.panels),
            self.sidebar.look_comparison(&other.sidebar),
            self.overlays.look_comparison(&other.overlays),
            self.menus.look_comparison(&other.menus),
        ];
        if self.scheme == other.scheme
            && layer_comparisons
                .iter()
                .all(|layer_comparison| *layer_comparison == LookComparison::Matching)
        {
            LookComparison::Matching
        } else {
            LookComparison::Differing
        }
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
    pub fn press_does(self) -> &'static str {
        match self {
            Self::ClearDark => "switch to the clear dark look",
            Self::BrightOffice => "switch to the bright office look",
            Self::FrostedDark => "switch to the frosted dark look",
        }
    }

    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::ClearDark => {
                "dark colours, see-through panels without outline, blur or shadow; a frosted, \
                 outlined sidebar with shadow"
            }
            Self::BrightOffice => {
                "light colours, nearly opaque panels without blur, blurred tooltips and sidebar"
            }
            Self::FrostedDark => "dark colours, translucent panels frosted with blur",
        }
    }

    #[must_use]
    pub fn applied_to(self, current: ShellAppearance) -> ShellAppearance {
        let shaped = self.shaped_layers(current);
        ShellAppearance {
            sidebar: LayerAppearance {
                backdrop_blur: BackdropBlur::Blurred,
                shadow: ShadowStyle {
                    casting: ShadowCasting::Cast,
                    ..shaped.sidebar.shadow
                },
                ..shaped.sidebar
            },
            overlays: LayerAppearance {
                backdrop_blur: BackdropBlur::Blurred,
                ..shaped.overlays
            },
            menus: LayerAppearance {
                backdrop_blur: BackdropBlur::Blurred,
                ..shaped.menus
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
                        casting: ShadowCasting::Omitted,
                        ..layer.shadow
                    },
                    ..layer
                };
                let panels = cleared(current.panels);
                let sidebar = cleared(current.sidebar);
                ShellAppearance {
                    scheme: ColorMode::Dark,
                    panels: LayerAppearance {
                        backdrop_blur: BackdropBlur::Clear,
                        outline: OutlineStyle {
                            tracing: OutlineTracing::Omitted,
                            ..panels.outline
                        },
                        ..panels
                    },
                    sidebar: LayerAppearance {
                        opacity: 0.55,
                        outline: OutlineStyle {
                            tracing: OutlineTracing::Drawn,
                            ..sidebar.outline
                        },
                        ..sidebar
                    },
                    overlays: LayerAppearance {
                        opacity: 0.75,
                        ..current.overlays
                    },
                    menus: LayerAppearance {
                        opacity: 0.75,
                        ..current.menus
                    },
                    ..current
                }
            }
            Self::BrightOffice => {
                let solid = |layer: LayerAppearance| LayerAppearance {
                    opacity: 0.95,
                    backdrop_blur: BackdropBlur::Clear,
                    ..layer
                };
                ShellAppearance {
                    scheme: ColorMode::Light,
                    panels: solid(current.panels),
                    sidebar: solid(current.sidebar),
                    overlays: solid(current.overlays),
                    menus: solid(current.menus),
                }
            }
            Self::FrostedDark => {
                let frosted = |layer: LayerAppearance, opacity: f32| LayerAppearance {
                    opacity,
                    backdrop_blur: BackdropBlur::Blurred,
                    ..layer
                };
                ShellAppearance {
                    scheme: ColorMode::Dark,
                    panels: frosted(current.panels, 0.8),
                    sidebar: frosted(current.sidebar, 0.9),
                    overlays: frosted(current.overlays, 0.7),
                    menus: frosted(current.menus, 0.7),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppearancePreset, ShellAppearance};
    use crate::{
        desktop_panels::region_reporter::BackdropBlur,
        widgets::shell_layer_appearance::{
            LayerAppearance, LookComparison, ShadowCasting, ShadowStyle,
        },
    };

    #[test]
    fn every_preset_blurs_and_shadows_the_sidebar() {
        let plain_sidebar = ShellAppearance {
            sidebar: LayerAppearance {
                backdrop_blur: BackdropBlur::Clear,
                shadow: ShadowStyle {
                    casting: ShadowCasting::Omitted,
                    ..ShellAppearance::default().sidebar.shadow
                },
                ..ShellAppearance::default().sidebar
            },
            ..ShellAppearance::default()
        };
        for preset in AppearancePreset::ALL {
            let applied = preset.applied_to(plain_sidebar);
            assert_eq!(
                applied.sidebar.backdrop_blur,
                BackdropBlur::Blurred,
                "{preset:?}"
            );
            assert_eq!(
                applied.sidebar.shadow.casting,
                ShadowCasting::Cast,
                "{preset:?}"
            );
        }
    }

    #[test]
    fn every_preset_blurs_behind_hover_cards() {
        let unblurred = ShellAppearance {
            overlays: LayerAppearance {
                backdrop_blur: BackdropBlur::Clear,
                ..ShellAppearance::default().overlays
            },
            ..ShellAppearance::default()
        };
        for preset in AppearancePreset::ALL {
            assert_eq!(
                preset.applied_to(unblurred).overlays.backdrop_blur,
                BackdropBlur::Blurred,
                "{preset:?}"
            );
        }
    }

    #[test]
    fn every_preset_is_shown_once_applied() {
        for preset in AppearancePreset::ALL {
            let applied = preset.applied_to(ShellAppearance::default());
            assert_eq!(
                preset.applied_to(applied).look_comparison(&applied),
                LookComparison::Matching,
                "{preset:?}"
            );
        }
    }

    #[test]
    fn only_the_applied_preset_is_shown() {
        let office = AppearancePreset::BrightOffice.applied_to(ShellAppearance::default());
        assert_eq!(
            AppearancePreset::ClearDark
                .applied_to(office)
                .look_comparison(&office),
            LookComparison::Differing
        );
        assert_eq!(
            AppearancePreset::FrostedDark
                .applied_to(office)
                .look_comparison(&office),
            LookComparison::Differing
        );
    }
}
