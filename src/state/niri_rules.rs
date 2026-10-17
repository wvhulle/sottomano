use std::{
    fmt, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use club_kdl::{KdlDeserialize, KdlSerialize};
use iced::Task;
use tokio::fs;

use crate::{data::source::niri_config::parse_niri_kdl, desktop_shell::monitor_surfaces::Position};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NiriLayout {
    pub window_corner_radius_px: f32,
    pub gaps_px: f32,
}

impl Default for NiriLayout {
    fn default() -> Self {
        Self {
            window_corner_radius_px: 0.0,
            gaps_px: 16.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NiriStyling {
    pub xray: bool,
    pub layout: NiriLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    Tooltip,
    Backdrop(Position),
}

impl StyledSurface {
    const ALL: [Self; 4] = [
        Self::Panel,
        Self::Tooltip,
        Self::Backdrop(Position::Top),
        Self::Backdrop(Position::Bottom),
    ];

    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "vetrata-panel",
            Self::Tooltip => "vetrata-tooltip",
            Self::Backdrop(Position::Top) => "vetrata-backdrop-top",
            Self::Backdrop(Position::Bottom) => "vetrata-backdrop-bottom",
            Self::Backdrop(Position::Left) => "vetrata-backdrop-left",
        }
    }

    fn rule(self, styling: NiriStyling) -> LayerRule {
        let matches = NamespaceMatch::from(self);
        match self {
            Self::Backdrop(position) => LayerRule {
                matches,
                background_effects: Vec::new(),
                shadows: vec![BackdropShadow::from(position)],
            },
            Self::Panel | Self::Tooltip => LayerRule {
                matches,
                background_effects: vec![BackgroundEffect {
                    blur: true,
                    xray: styling.xray,
                }],
                shadows: Vec::new(),
            },
        }
    }
}

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
struct VetrataRules {
    #[kdl(children)]
    layer_rules: Vec<LayerRule>,
    #[kdl(child)]
    layout: LayoutSection,
    #[kdl(child)]
    window_rule: WindowRule,
}

impl FromStr for VetrataRules {
    type Err = RulesError;

    fn from_str(text: &str) -> Result<Self, RulesError> {
        Ok(club_kdl::from_kdl_document(&parse_niri_kdl(text)?)?)
    }
}

impl VetrataRules {
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
    on: ShadowOn,
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
            on: ShadowOn {},
            softness: 27,
            spread: 25,
            color: "#00000060".to_owned(),
            offset: ShadowOffset { x: 0, y: lift },
        }
    }
}

#[derive(Debug, PartialEq, KdlDeserialize, KdlSerialize)]
#[kdl(name = "on")]
struct ShadowOn {}

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
    #[kdl(child(name = "geometry-corner-radius"), unwrap_arg)]
    geometry_corner_radius: f32,
    #[kdl(child(name = "clip-to-geometry"), unwrap_arg)]
    clip_to_geometry: bool,
}

impl From<NiriStyling> for VetrataRules {
    fn from(styling: NiriStyling) -> Self {
        Self {
            layer_rules: StyledSurface::ALL
                .into_iter()
                .map(|surface| surface.rule(styling))
                .collect(),
            layout: LayoutSection {
                gaps: styling.layout.gaps_px,
            },
            window_rule: WindowRule {
                geometry_corner_radius: styling.layout.window_corner_radius_px,
                clip_to_geometry: true,
            },
        }
    }
}

impl From<VetrataRules> for NiriStyling {
    fn from(rules: VetrataRules) -> Self {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        Self {
            xray: rules
                .layer_rules
                .into_iter()
                .filter(|rule| rule.matches == panel)
                .flat_map(|rule| rule.background_effects)
                .any(|effect| effect.xray),
            layout: NiriLayout {
                window_corner_radius_px: rules.window_rule.geometry_corner_radius,
                gaps_px: rules.layout.gaps,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredRules(String);

#[derive(Clone)]
enum Written {
    Unread,
    Known(String),
}

#[derive(Clone)]
pub struct NiriRulesFile {
    path: PathBuf,
    written: Written,
    styling: NiriStyling,
}

impl NiriRulesFile {
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: config_home.join("vetrata").join("niri.kdl"),
            written: Written::Unread,
            styling: NiriStyling::default(),
        }
    }

    pub fn styling(&self) -> NiriStyling {
        self.styling
    }

    pub fn restyle(&mut self, change: impl FnOnce(&mut NiriStyling)) -> Task<bool> {
        change(&mut self.styling);
        self.write()
    }

    pub fn read(&self) -> Task<StoredRules> {
        let path = self.path.clone();
        Task::future(
            async move { StoredRules(fs::read_to_string(&path).await.unwrap_or_default()) },
        )
    }

    pub fn adopt(&mut self, stored: StoredRules) -> Task<bool> {
        if !stored.0.is_empty() {
            match stored.0.parse::<VetrataRules>() {
                Ok(rules) => self.styling = NiriStyling::from(rules),
                Err(e) => tracing::warn!("niri rules: keeping the current look, file is {e}"),
            }
        }
        self.written = Written::Known(stored.0);
        self.write()
    }

    fn write(&mut self) -> Task<bool> {
        let Written::Known(last) = &self.written else {
            return Task::done(false);
        };
        let rules = match VetrataRules::from(self.styling).to_kdl_v1() {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("niri rules: rendering failed: {e}");
                return Task::done(false);
            }
        };
        if rules == *last {
            return Task::done(true);
        }
        self.written = Written::Known(rules.clone());
        let path = self.path.clone();
        Task::future(async move {
            replace(&path, &rules)
                .await
                .inspect_err(|e| {
                    tracing::warn!("niri rules: writing {} failed: {e}", path.display());
                })
                .is_ok()
        })
    }
}

async fn replace(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).await?;
    }
    let staged = path.with_extension("kdl.staged");
    fs::write(&staged, contents).await?;
    fs::rename(&staged, path).await
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        BackgroundEffect, NamespaceMatch, NiriLayout, NiriRulesFile, NiriStyling, StoredRules,
        StyledSurface, VetrataRules,
    };

    fn styled() -> NiriStyling {
        NiriStyling {
            xray: true,
            layout: NiriLayout {
                window_corner_radius_px: 7.5,
                gaps_px: 5.0,
            },
        }
    }

    #[test]
    fn a_rendered_look_reads_back_unchanged() {
        let written = styled();
        let text = VetrataRules::from(written).to_kdl_v1().unwrap();
        let read = NiriStyling::from(text.parse::<VetrataRules>().unwrap());
        assert_eq!(read, written);
    }

    #[test]
    fn an_empty_file_keeps_the_defaults() {
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        let _ = rules.adopt(StoredRules(String::new()));
        assert_eq!(rules.styling(), NiriStyling::default());
    }

    #[test]
    fn the_panel_leaves_shadows_and_corners_to_vetrata() {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let rules = VetrataRules::from(styled());
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
