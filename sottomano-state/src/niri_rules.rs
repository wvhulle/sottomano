use std::{
    fmt, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use club_kdl::{KdlDeserialize, KdlSerialize};
use sottomano_config::screen_edges::Position;
use sottomano_sources::desktop_windows_apps::niri_kdl_config::parse_niri_kdl;
use tokio::fs;

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
    NotificationToasts,
    Backdrop(Position),
}

impl StyledSurface {
    const ALL: [Self; 5] = [
        Self::Panel,
        Self::Tooltip,
        Self::NotificationToasts,
        Self::Backdrop(Position::Top),
        Self::Backdrop(Position::Bottom),
    ];

    #[must_use]
    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "sottomano-panel",
            Self::Tooltip => "sottomano-tooltip",
            Self::NotificationToasts => "sottomano-notification-toasts",
            Self::Backdrop(Position::Top) => "sottomano-backdrop-top",
            Self::Backdrop(Position::Bottom) => "sottomano-backdrop-bottom",
            Self::Backdrop(Position::Left) => "sottomano-backdrop-left",
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
            Self::Panel | Self::Tooltip | Self::NotificationToasts => LayerRule {
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
    #[kdl(child)]
    matches: Option<WindowMatch>,
    #[kdl(child(name = "geometry-corner-radius"), unwrap_arg)]
    geometry_corner_radius: Option<f32>,
    #[kdl(child(name = "clip-to-geometry"), unwrap_arg)]
    clip_to_geometry: Option<bool>,
    #[kdl(child)]
    border: Option<HeatedBorder>,
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
struct HeatedBorder {
    #[kdl(child(name = "active-color"), unwrap_arg)]
    active_color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatedWindow {
    pub app_id: Option<String>,
    pub title: String,
    pub active_color: String,
}

impl From<&HeatedWindow> for WindowRule {
    fn from(heated: &HeatedWindow) -> Self {
        Self {
            matches: Some(WindowMatch {
                app_id: heated.app_id.as_deref().map(exact_regex),
                title: exact_regex(&heated.title),
            }),
            geometry_corner_radius: None,
            clip_to_geometry: None,
            border: Some(HeatedBorder {
                active_color: heated.active_color.clone(),
            }),
        }
    }
}

fn exact_regex(literal: &str) -> String {
    let escaped: String = literal
        .chars()
        .filter(|c| !c.is_control())
        .flat_map(|c| {
            let metacharacter = "\\.+*?()|[]{}^$#&-~".contains(c);
            metacharacter.then_some('\\').into_iter().chain([c])
        })
        .collect();
    format!("^{escaped}$")
}

impl SottomanoRules {
    fn styled(styling: NiriStyling, heated_windows: &[HeatedWindow]) -> Self {
        let geometry_rule = WindowRule {
            matches: None,
            geometry_corner_radius: Some(styling.layout.window_corner_radius_px),
            clip_to_geometry: Some(true),
            border: None,
        };
        Self {
            layer_rules: StyledSurface::ALL
                .into_iter()
                .map(|surface| surface.rule(styling))
                .collect(),
            layout: LayoutSection {
                gaps: styling.layout.gaps_px,
            },
            window_rules: [geometry_rule]
                .into_iter()
                .chain(heated_windows.iter().map(WindowRule::from))
                .collect(),
        }
    }
}

impl From<SottomanoRules> for NiriStyling {
    fn from(rules: SottomanoRules) -> Self {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let defaults = NiriLayout::default();
        Self {
            xray: rules
                .layer_rules
                .into_iter()
                .filter(|rule| rule.matches == panel)
                .flat_map(|rule| rule.background_effects)
                .any(|effect| effect.xray),
            layout: NiriLayout {
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

#[derive(Debug, Clone)]
pub enum StoredRules {
    Absent,
    Text(String),
    Unreadable(io::ErrorKind),
}

#[derive(Debug, Clone, Copy)]
pub struct UnreadableRules<'a> {
    pub path: &'a Path,
    pub kind: io::ErrorKind,
}

#[derive(Clone)]
enum Written {
    Unread,
    Unreadable(io::ErrorKind),
    Absent,
    Known(String),
}

#[derive(Clone)]
pub struct NiriRulesFile {
    path: PathBuf,
    written: Written,
    styling: NiriStyling,
    heated_windows: Vec<HeatedWindow>,
}

impl NiriRulesFile {
    #[must_use]
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: config_home.join("sottomano").join("niri.kdl"),
            written: Written::Unread,
            styling: NiriStyling::default(),
            heated_windows: Vec::new(),
        }
    }

    pub fn reheat(
        &mut self,
        heated_windows: Vec<HeatedWindow>,
    ) -> Option<impl Future<Output = bool> + Send + use<>> {
        if heated_windows == self.heated_windows {
            return None;
        }
        self.heated_windows = heated_windows;
        Some(self.write().persisted())
    }

    #[must_use]
    pub fn styling(&self) -> NiriStyling {
        self.styling
    }

    #[must_use]
    pub fn unreadable(&self) -> Option<UnreadableRules<'_>> {
        match self.written {
            Written::Unreadable(kind) => Some(UnreadableRules {
                path: &self.path,
                kind,
            }),
            Written::Unread | Written::Absent | Written::Known(_) => None,
        }
    }

    pub fn restyle<Change: FnOnce(&mut NiriStyling)>(
        &mut self,
        change: Change,
    ) -> impl Future<Output = bool> + Send + use<Change> {
        change(&mut self.styling);
        self.write().persisted()
    }

    pub fn read(&self) -> impl Future<Output = StoredRules> + Send + use<> {
        let path = self.path.clone();
        async move {
            match fs::read_to_string(&path).await {
                Ok(text) => StoredRules::Text(text),
                Err(e) if e.kind() == io::ErrorKind::NotFound => StoredRules::Absent,
                Err(e) => {
                    tracing::warn!("niri rules: reading {} failed: {e}", path.display());
                    StoredRules::Unreadable(e.kind())
                }
            }
        }
    }

    pub fn adopt(&mut self, stored: StoredRules) -> impl Future<Output = bool> + Send + use<> {
        self.written = match stored {
            StoredRules::Unreadable(kind) => Written::Unreadable(kind),
            StoredRules::Absent => Written::Absent,
            StoredRules::Text(text) => {
                match text.parse::<SottomanoRules>() {
                    Ok(rules) => self.styling = NiriStyling::from(rules),
                    Err(e) => tracing::warn!("niri rules: keeping the current look, file is {e}"),
                }
                Written::Known(text)
            }
        };
        self.write().persisted()
    }

    fn write(&mut self) -> RulesWrite {
        if matches!(self.written, Written::Unread | Written::Unreadable(_)) {
            return RulesWrite::Withheld;
        }
        let rules = match SottomanoRules::styled(self.styling, &self.heated_windows).to_kdl_v1() {
            Ok(rules) => rules,
            Err(e) => {
                tracing::warn!("niri rules: rendering failed: {e}");
                return RulesWrite::Withheld;
            }
        };
        if matches!(&self.written, Written::Known(last) if *last == rules) {
            return RulesWrite::AlreadyOnDisk;
        }
        self.written = Written::Known(rules.clone());
        RulesWrite::Replace {
            path: self.path.clone(),
            rules,
        }
    }
}

enum RulesWrite {
    Withheld,
    AlreadyOnDisk,
    Replace { path: PathBuf, rules: String },
}

impl RulesWrite {
    async fn persisted(self) -> bool {
        match self {
            Self::Withheld => false,
            Self::AlreadyOnDisk => true,
            Self::Replace { path, rules } => replace(&path, &rules)
                .await
                .inspect_err(|e| {
                    tracing::warn!("niri rules: writing {} failed: {e}", path.display());
                })
                .is_ok(),
        }
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
        BackgroundEffect, HeatedWindow, NamespaceMatch, NiriLayout, NiriRulesFile, NiriStyling,
        SottomanoRules, StoredRules, StyledSurface,
    };

    fn heated_terminal() -> HeatedWindow {
        HeatedWindow {
            app_id: Some("kitty".to_owned()),
            title: "nvim (main) \"notes\".md".to_owned(),
            active_color: "#ffb454ff".to_owned(),
        }
    }

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
        let text = SottomanoRules::styled(written, &[]).to_kdl_v1().unwrap();
        let read = NiriStyling::from(text.parse::<SottomanoRules>().unwrap());
        assert_eq!(read, written);
    }

    #[test]
    fn heated_windows_leave_the_look_unchanged_on_reading_back() {
        let written = styled();
        let text = SottomanoRules::styled(written, &[heated_terminal()])
            .to_kdl_v1()
            .unwrap();
        let read = NiriStyling::from(text.parse::<SottomanoRules>().unwrap());
        assert_eq!(read, written);
    }

    #[test]
    fn a_heated_window_is_matched_by_its_escaped_title() {
        let rules = SottomanoRules::styled(styled(), &[heated_terminal()]);
        let heated_match = rules.window_rules[1].matches.as_ref().unwrap();
        assert_eq!(heated_match.app_id.as_deref(), Some("^kitty$"));
        assert_eq!(heated_match.title, "^nvim \\(main\\) \"notes\"\\.md$");
    }

    #[test]
    fn an_unchanged_heat_is_not_rewritten() {
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        assert!(rules.reheat(vec![heated_terminal()]).is_some());
        assert!(rules.reheat(vec![heated_terminal()]).is_none());
    }

    #[test]
    fn an_absent_file_keeps_the_defaults() {
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        drop(rules.adopt(StoredRules::Absent));
        assert_eq!(rules.styling(), NiriStyling::default());
    }

    #[test]
    fn the_panel_leaves_shadows_and_corners_to_sottomano() {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let rules = SottomanoRules::styled(styled(), &[]);
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
