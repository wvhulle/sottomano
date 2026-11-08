use std::{fmt, str::FromStr};

use club_kdl::{KdlDeserialize, KdlSerialize};
use sottomano_desktop_sources::compositor_window_ipc::{
    compositor_styling_model::{
        AppWindowLayout, CompositorStyling, HeatedWindow, ShellBlurDepth, StyledSurface,
        SwitchHeat, escaped_for_regex,
    },
    niri_kdl_config::parse_niri_kdl,
};
use sottomano_toml_config::screen_edges::Position;

#[derive(Debug)]
enum RulesError {
    Syntax(kdl::KdlError),
    Schema(club_kdl::Error),
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(e) => write!(f, "not valid KDL: {e}"),
            Self::Schema(e) => write!(f, "not the expected rules: {e}"),
        }
    }
}

impl From<kdl::KdlError> for RulesError {
    fn from(e: kdl::KdlError) -> Self {
        Self::Syntax(e)
    }
}

impl From<club_kdl::Error> for RulesError {
    fn from(e: club_kdl::Error) -> Self {
        Self::Schema(e)
    }
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(document)]
struct SottomanoRules {
    #[kdl(children)]
    layer_rules: Vec<LayerRule>,
    #[kdl(child)]
    layout: LayoutSection,
    #[kdl(children)]
    window_rules: Vec<WindowRule>,
}

impl FromStr for SottomanoRules {
    type Err = RulesError;

    fn from_str(text: &str) -> Result<Self, RulesError> {
        Ok(club_kdl::from_kdl_document(&parse_niri_kdl(text)?)?)
    }
}

impl SottomanoRules {
    fn to_kdl_v1(&self) -> club_kdl::Result<String> {
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
        let lift = match position {
            Position::Top => -3,
            Position::Bottom => 3,
            Position::Left => 0,
        };
        Self {
            on: SwitchedOn {},
            softness: 27,
            spread: 25,
            color: "#00000060".to_owned(),
            offset: ShadowOffset { x: 0, y: lift },
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

fn focus_ring_of(switch_heat: &SwitchHeat) -> Option<FocusRing> {
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

fn neutral_border_of(switch_heat: &SwitchHeat) -> Option<UniformBorder> {
    match switch_heat {
        SwitchHeat::Hidden => None,
        SwitchHeat::Shown {
            neutral_border_color,
            ..
        } => Some(UniformBorder::colored(neutral_border_color)),
    }
}

fn layer_rule_of(surface: StyledSurface, styling: CompositorStyling) -> LayerRule {
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

impl From<&HeatedWindow> for WindowRule {
    fn from(heated: &HeatedWindow) -> Self {
        Self {
            matches: Some(WindowMatch {
                app_id: heated
                    .app_id()
                    .map(|app_id| format!("^{}$", escaped_for_regex(app_id))),
                title: format!(
                    "^\\W*{}$",
                    escaped_for_regex(heated.title_without_status_prefix())
                ),
            }),
            geometry_corner_radius: None,
            clip_to_geometry: None,
            focus_ring: None,
            border: Some(UniformBorder::colored(heated.border_color())),
        }
    }
}

impl SottomanoRules {
    fn styled(styling: CompositorStyling, switch_heat: &SwitchHeat) -> Self {
        let every_window_rule = WindowRule {
            matches: None,
            geometry_corner_radius: Some(styling.layout.window_corner_radius_px),
            clip_to_geometry: Some(true),
            focus_ring: focus_ring_of(switch_heat),
            border: neutral_border_of(switch_heat),
        };
        Self {
            layer_rules: StyledSurface::ALL
                .into_iter()
                .map(|surface| layer_rule_of(surface, styling))
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

impl From<SottomanoRules> for CompositorStyling {
    fn from(rules: SottomanoRules) -> Self {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let defaults = AppWindowLayout::default();
        Self {
            blur_depth: if rules
                .layer_rules
                .into_iter()
                .filter(|rule| rule.matches == panel)
                .flat_map(|rule| rule.background_effects)
                .any(|effect| effect.xray)
            {
                ShellBlurDepth::WallpaperOnly
            } else {
                ShellBlurDepth::WindowsBehind
            },
            layout: AppWindowLayout {
                window_corner_radius_px: rules
                    .window_rules
                    .iter()
                    .filter(|rule| rule.matches.is_none())
                    .find_map(|rule| rule.geometry_corner_radius)
                    .unwrap_or(defaults.window_corner_radius_px),
                gaps_px: rules.layout.gaps,
            },
        }
    }
}

pub fn rendered_niri_rules(
    styling: CompositorStyling,
    switch_heat: &SwitchHeat,
) -> Result<String, club_kdl::Error> {
    SottomanoRules::styled(styling, switch_heat).to_kdl_v1()
}

#[must_use]
pub fn imported_niri_styling(text: &str) -> Option<CompositorStyling> {
    text.parse::<SottomanoRules>()
        .inspect_err(|parse_error| {
            tracing::warn!("niri rules: not imported, the file is {parse_error}");
        })
        .ok()
        .map(CompositorStyling::from)
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::compositor_window_ipc::compositor_styling_model::{
        AppWindowLayout, CompositorStyling, HeatedWindow, ShellBlurDepth, StyledSurface, SwitchHeat,
    };

    use super::{BackgroundEffect, NamespaceMatch, SottomanoRules};

    fn heated_terminal_titled(title: &str) -> SwitchHeat {
        SwitchHeat::Shown {
            focus_ring_color: "#95e6cb".to_owned(),
            neutral_border_color: "#0b0e14".to_owned(),
            windows: HeatedWindow::titled(Some("kitty".to_owned()), title, "#ffb454".to_owned())
                .into_iter()
                .collect(),
        }
    }

    fn heated_terminal() -> SwitchHeat {
        heated_terminal_titled("nvim (main) \"notes\".md")
    }

    fn styled() -> CompositorStyling {
        CompositorStyling {
            blur_depth: ShellBlurDepth::WallpaperOnly,
            layout: AppWindowLayout {
                window_corner_radius_px: 7.5,
                gaps_px: 5.0,
            },
        }
    }

    #[test]
    fn a_rendered_look_reads_back_unchanged() {
        let written = styled();
        let text = SottomanoRules::styled(written, &SwitchHeat::Hidden)
            .to_kdl_v1()
            .unwrap();
        let read = CompositorStyling::from(text.parse::<SottomanoRules>().unwrap());
        assert_eq!(read, written);
    }

    #[test]
    fn heated_windows_leave_the_look_unchanged_on_reading_back() {
        let written = styled();
        let text = SottomanoRules::styled(written, &heated_terminal())
            .to_kdl_v1()
            .unwrap();
        let read = CompositorStyling::from(text.parse::<SottomanoRules>().unwrap());
        assert_eq!(read, written);
    }

    #[test]
    fn a_heated_window_is_matched_by_its_escaped_title() {
        let rules = SottomanoRules::styled(styled(), &heated_terminal());
        let heated_match = rules.window_rules[1].matches.as_ref().unwrap();
        assert_eq!(heated_match.app_id.as_deref(), Some("^kitty$"));
        assert_eq!(heated_match.title, "^\\W*nvim \\(main\\) \"notes\"\\.md$");
    }

    #[test]
    fn a_spinning_status_prefix_keeps_the_same_rule() {
        assert_eq!(
            heated_terminal_titled("✳ enable-flash-attention"),
            heated_terminal_titled("◐ enable-flash-attention")
        );
    }

    #[test]
    fn every_window_gets_the_neutral_border_while_heat_is_shown() {
        let shown = SottomanoRules::styled(styled(), &heated_terminal());
        let neutral = shown.window_rules[0].border.as_ref().unwrap();
        assert_eq!(neutral.active_color, "#0b0e14");
        assert_eq!(neutral.inactive_color, "#0b0e14");
        let hidden = SottomanoRules::styled(styled(), &SwitchHeat::Hidden);
        assert!(hidden.window_rules[0].border.is_none());
    }

    #[test]
    fn the_focus_ring_marks_focus_only_while_heat_is_shown() {
        let shown = SottomanoRules::styled(styled(), &heated_terminal());
        let hidden = SottomanoRules::styled(styled(), &SwitchHeat::Hidden);
        assert!(shown.window_rules[0].focus_ring.is_some());
        assert!(hidden.window_rules[0].focus_ring.is_none());
        assert_eq!(hidden.window_rules.len(), 1);
    }

    #[test]
    fn the_panel_leaves_shadows_and_corners_to_sottomano() {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let rules = SottomanoRules::styled(styled(), &SwitchHeat::Hidden);
        let panel_rule = rules
            .layer_rules
            .iter()
            .find(|rule| rule.matches == panel)
            .expect("panel rule");
        assert!(matches!(
            panel_rule.background_effects.as_slice(),
            [BackgroundEffect { blur: true, .. }]
        ));
        assert!(panel_rule.shadows.is_empty());
    }
}
