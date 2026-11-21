use club_kdl::{KdlDeserialize, KdlSerialize};
use sottomano_compositor_model::{
    compositor_styling_model::{
        CompositorStyling, HeatedWindow, PANEL_EDGE_SHADOW, ShellBlurDepth, StyledSurface,
        SwitchHeat, escaped_for_regex,
    },
    compositor_window_facts::WindowAppId,
};
use sottomano_toml_config::screen_edges::Position;

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(document)]
pub(crate) struct SottomanoRules {
    #[kdl(children)]
    layer_rules: Vec<LayerRule>,
    #[kdl(child)]
    layout: LayoutSection,
    #[kdl(children)]
    window_rules: Vec<WindowRule>,
}

impl SottomanoRules {
    pub(crate) fn to_kdl_v1(&self) -> club_kdl::Result<String> {
        let mut document = club_kdl::to_kdl_document(self)?;
        document.autoformat();
        document.ensure_v1();
        Ok(document.to_string())
    }
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "layer-rule")]
struct LayerRule {
    #[kdl(child)]
    matches: NamespaceMatch,
    #[kdl(children)]
    background_effects: Vec<BackgroundEffect>,
    #[kdl(children)]
    shadows: Vec<BackdropShadow>,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "match")]
struct NamespaceMatch {
    #[kdl(property)]
    namespace: String,
}

impl From<StyledSurface> for NamespaceMatch {
    fn from(surface: StyledSurface) -> Self {
        Self {
            namespace: format!("^{}$", surface.namespace()),
        }
    }
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "background-effect")]
struct BackgroundEffect {
    #[kdl(child, unwrap_arg)]
    blur: bool,
    #[kdl(child, unwrap_arg)]
    xray: bool,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "shadow")]
struct BackdropShadow {
    #[kdl(child)]
    on: SwitchedOn,
    #[kdl(child, unwrap_arg)]
    softness: i64,
    #[kdl(child, unwrap_arg)]
    spread: i64,
    #[kdl(child, unwrap_arg)]
    color: String,
    #[kdl(child)]
    offset: ShadowOffset,
}

impl From<Position> for BackdropShadow {
    fn from(position: Position) -> Self {
        let lift = PANEL_EDGE_SHADOW.lift_px as i64;
        let offscreen_lift = match position {
            Position::Top => -lift,
            Position::Bottom => lift,
            Position::Left => 0,
        };
        Self {
            on: SwitchedOn {},
            softness: PANEL_EDGE_SHADOW.softness_px as i64,
            spread: PANEL_EDGE_SHADOW.spread_px as i64,
            color: format!("#000000{:02x}", PANEL_EDGE_SHADOW.alpha),
            offset: ShadowOffset {
                x: 0,
                y: offscreen_lift,
            },
        }
    }
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "on")]
struct SwitchedOn {}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "offset")]
struct ShadowOffset {
    #[kdl(property)]
    x: i64,
    #[kdl(property)]
    y: i64,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "layout")]
struct LayoutSection {
    #[kdl(child, unwrap_arg)]
    gaps: f32,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "window-rule")]
struct WindowRule {
    #[kdl(child)]
    matches: Option<WindowMatch>,
    #[kdl(child(name = "geometry-corner-radius"), unwrap_arg)]
    geometry_corner_radius: Option<f32>,
    #[kdl(child(name = "clip-to-geometry"), unwrap_arg)]
    clip_to_geometry: Option<bool>,
    #[kdl(child)]
    focus_ring: Option<FocusRing>,
    #[kdl(child)]
    border: Option<UniformBorder>,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "focus-ring")]
struct FocusRing {
    #[kdl(child)]
    on: SwitchedOn,
    #[kdl(child(name = "active-color"), unwrap_arg)]
    active_color: String,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "match")]
struct WindowMatch {
    #[kdl(property(rename = "app-id"))]
    app_id: Option<String>,
    #[kdl(property)]
    title: String,
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "border")]
struct UniformBorder {
    #[kdl(child(name = "active-color"), unwrap_arg)]
    active_color: String,
    #[kdl(child(name = "inactive-color"), unwrap_arg)]
    inactive_color: String,
}

impl UniformBorder {
    fn colored(color: &str) -> Self {
        Self {
            active_color: color.to_owned(),
            inactive_color: color.to_owned(),
        }
    }
}

impl FocusRing {
    fn while_heat_shown(switch_heat: &SwitchHeat) -> Option<Self> {
        match switch_heat {
            SwitchHeat::Hidden => None,
            SwitchHeat::Shown {
                focus_ring_color, ..
            } => Some(FocusRing {
                on: SwitchedOn {},
                active_color: focus_ring_color.clone(),
            }),
        }
    }
}

impl UniformBorder {
    fn neutral_while_heat_shown(switch_heat: &SwitchHeat) -> Option<Self> {
        match switch_heat {
            SwitchHeat::Hidden => None,
            SwitchHeat::Shown {
                neutral_border_color,
                ..
            } => Some(UniformBorder::colored(neutral_border_color)),
        }
    }
}

impl LayerRule {
    fn styled(surface: StyledSurface, styling: CompositorStyling) -> Self {
        let matches = NamespaceMatch::from(surface);
        match surface {
            StyledSurface::Backdrop(position) => LayerRule {
                matches,
                background_effects: Vec::new(),
                shadows: vec![BackdropShadow::from(position)],
            },
            StyledSurface::Panel | StyledSurface::NotificationToasts => LayerRule {
                matches,
                background_effects: vec![BackgroundEffect {
                    blur: true,
                    xray: styling.blur_depth == ShellBlurDepth::WallpaperOnly,
                }],
                shadows: Vec::new(),
            },
        }
    }
}

impl From<&HeatedWindow> for WindowRule {
    fn from(heated: &HeatedWindow) -> Self {
        Self {
            matches: Some(WindowMatch {
                app_id: match &heated.app_id {
                    WindowAppId::Reported(app_id) => {
                        Some(format!("^{}$", escaped_for_regex(app_id)))
                    }
                    WindowAppId::Unreported => None,
                },
                title: format!(
                    "^\\W*{}$",
                    escaped_for_regex(&heated.title_without_status_prefix)
                ),
            }),
            geometry_corner_radius: None,
            clip_to_geometry: None,
            focus_ring: None,
            border: Some(UniformBorder::colored(&heated.border_color)),
        }
    }
}

impl SottomanoRules {
    pub(crate) fn styled(styling: CompositorStyling, switch_heat: &SwitchHeat) -> Self {
        let every_window_rule = WindowRule {
            matches: None,
            geometry_corner_radius: Some(styling.layout.window_corner_radius_px),
            clip_to_geometry: Some(true),
            focus_ring: FocusRing::while_heat_shown(switch_heat),
            border: UniformBorder::neutral_while_heat_shown(switch_heat),
        };
        Self {
            layer_rules: StyledSurface::ALL
                .into_iter()
                .map(|surface| LayerRule::styled(surface, styling))
                .collect(),
            layout: LayoutSection {
                gaps: styling.layout.gaps_px,
            },
            window_rules: [every_window_rule]
                .into_iter()
                .chain(switch_heat.heated_windows().iter().map(WindowRule::from))
                .collect(),
        }
    }
}

#[cfg(test)]
mod niri_kdl_rules_tests;
