use std::{
    error::Error,
    fmt, io, mem,
    path::{Path, PathBuf},
    str::FromStr,
};

use club_kdl::{KdlDeserialize, KdlSerialize};
use sottomano_desktop_sources::desktop_windows_apps::niri_kdl_config::parse_niri_kdl;
use sottomano_toml_config::screen_edges::Position;
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
pub struct NiriStyling {
    pub blur_depth: ShellBlurDepth,
    pub layout: NiriLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    NotificationToasts,
    Backdrop(Position),
}

impl StyledSurface {
    const ALL: [Self; 4] = [
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

    fn rule(self, styling: NiriStyling) -> LayerRule {
        let matches = NamespaceMatch::from(self);
        match self {
            Self::Backdrop(position) => LayerRule {
                matches,
                background_effects: Vec::new(),
                shadows: vec![BackdropShadow::from(position)],
            },
            Self::Panel | Self::NotificationToasts => LayerRule {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatedWindow {
    app_id: Option<String>,
    title_without_status_prefix: String,
    border_color: String,
}

impl HeatedWindow {
    #[must_use]
    pub fn titled(app_id: Option<String>, title: &str, border_color: String) -> Option<Self> {
        let title_without_status_prefix = title.trim_start_matches(|c: char| !c.is_alphanumeric());
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
    fn focus_ring(&self) -> Option<FocusRing> {
        match self {
            Self::Hidden => None,
            Self::Shown {
                focus_ring_color, ..
            } => Some(FocusRing {
                on: SwitchedOn {},
                active_color: focus_ring_color.clone(),
            }),
        }
    }

    fn neutral_border(&self) -> Option<UniformBorder> {
        match self {
            Self::Hidden => None,
            Self::Shown {
                neutral_border_color,
                ..
            } => Some(UniformBorder::colored(neutral_border_color)),
        }
    }

    fn heated_windows(&self) -> &[HeatedWindow] {
        match self {
            Self::Hidden => &[],
            Self::Shown { windows, .. } => windows,
        }
    }
}

impl From<&HeatedWindow> for WindowRule {
    fn from(heated: &HeatedWindow) -> Self {
        Self {
            matches: Some(WindowMatch {
                app_id: heated
                    .app_id
                    .as_deref()
                    .map(|app_id| format!("^{}$", escaped_for_regex(app_id))),
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

fn escaped_for_regex(literal: &str) -> String {
    literal
        .chars()
        .filter(|c| !c.is_control())
        .flat_map(|c| {
            let metacharacter = "\\.+*?()|[]{}^$#&-~".contains(c);
            metacharacter.then_some('\\').into_iter().chain([c])
        })
        .collect()
}

impl SottomanoRules {
    fn styled(styling: NiriStyling, switch_heat: &SwitchHeat) -> Self {
        let every_window_rule = WindowRule {
            matches: None,
            geometry_corner_radius: Some(styling.layout.window_corner_radius_px),
            clip_to_geometry: Some(true),
            focus_ring: switch_heat.focus_ring(),
            border: switch_heat.neutral_border(),
        };
        Self {
            layer_rules: StyledSurface::ALL
                .into_iter()
                .map(|surface| surface.rule(styling))
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

impl From<SottomanoRules> for NiriStyling {
    fn from(rules: SottomanoRules) -> Self {
        let panel = NamespaceMatch::from(StyledSurface::Panel);
        let defaults = NiriLayout::default();
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
enum WriteSlot {
    Idle,
    Writing { rules: String },
    WritingThenNext { rules: String, next_rules: String },
}

#[derive(Debug)]
pub struct RulesWrite {
    path: PathBuf,
    rules: String,
}

impl RulesWrite {
    pub async fn run(self) -> Result<(), io::ErrorKind> {
        replace(&self.path, &self.rules)
            .await
            .map_err(|write_error| write_error.kind())
    }
}

#[derive(Debug)]
pub enum SaveStep {
    NothingInFlight,
    WriteInFlight,
    StartWrite(RulesWrite),
}

#[derive(Debug)]
pub enum RulesSaveError {
    NotYetRead,
    Unreadable(io::ErrorKind),
    Rendering(club_kdl::Error),
}

impl Error for RulesSaveError {}

impl fmt::Display for RulesSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotYetRead => write!(formatter, "the stored rules are not read yet"),
            Self::Unreadable(kind) => write!(formatter, "the stored rules are unreadable: {kind}"),
            Self::Rendering(render_error) => write!(formatter, "rendering failed: {render_error}"),
        }
    }
}

pub struct AdoptedRules {
    pub styling: NiriStyling,
    pub saved: Result<SaveStep, RulesSaveError>,
}

#[derive(Clone)]
pub struct NiriRulesFile {
    path: PathBuf,
    written: Written,
    write_slot: WriteSlot,
    switch_heat: SwitchHeat,
}

impl NiriRulesFile {
    #[must_use]
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: config_home.join("sottomano").join("niri.kdl"),
            written: Written::Unread,
            write_slot: WriteSlot::Idle,
            switch_heat: SwitchHeat::default(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reheat(
        &mut self,
        styling: NiriStyling,
        switch_heat: SwitchHeat,
    ) -> Result<SaveStep, RulesSaveError> {
        if switch_heat == self.switch_heat {
            return Ok(self.current_step());
        }
        self.switch_heat = switch_heat;
        self.save(styling)
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

    pub fn adopt(&mut self, stored: StoredRules, current_styling: NiriStyling) -> AdoptedRules {
        let mut styling = current_styling;
        self.written = match stored {
            StoredRules::Unreadable(kind) => Written::Unreadable(kind),
            StoredRules::Absent => Written::Absent,
            StoredRules::Text(text) => {
                match text.parse::<SottomanoRules>() {
                    Ok(rules) => styling = NiriStyling::from(rules),
                    Err(e) => tracing::warn!("niri rules: keeping the current look, file is {e}"),
                }
                Written::Known(text)
            }
        };
        AdoptedRules {
            styling,
            saved: self.save(styling),
        }
    }

    pub fn save(&mut self, styling: NiriStyling) -> Result<SaveStep, RulesSaveError> {
        match self.written {
            Written::Unread => return Err(RulesSaveError::NotYetRead),
            Written::Unreadable(kind) => return Err(RulesSaveError::Unreadable(kind)),
            Written::Absent | Written::Known(_) => {}
        }
        let rules = SottomanoRules::styled(styling, &self.switch_heat)
            .to_kdl_v1()
            .map_err(RulesSaveError::Rendering)?;
        if self.newest_rules() == Some(rules.as_str()) {
            return Ok(self.current_step());
        }
        Ok(match mem::replace(&mut self.write_slot, WriteSlot::Idle) {
            WriteSlot::Idle => self.start_write(rules),
            WriteSlot::Writing { rules: in_flight }
            | WriteSlot::WritingThenNext {
                rules: in_flight, ..
            } => {
                self.write_slot = WriteSlot::WritingThenNext {
                    rules: in_flight,
                    next_rules: rules,
                };
                SaveStep::WriteInFlight
            }
        })
    }

    pub fn finish_write(&mut self, written: Result<(), io::ErrorKind>) -> SaveStep {
        let (finished_rules, waiting_rules) =
            match mem::replace(&mut self.write_slot, WriteSlot::Idle) {
                WriteSlot::Idle => return SaveStep::NothingInFlight,
                WriteSlot::Writing { rules } => (rules, None),
                WriteSlot::WritingThenNext { rules, next_rules } => (rules, Some(next_rules)),
            };
        if written.is_ok() {
            self.written = Written::Known(finished_rules);
        }
        waiting_rules.map_or(SaveStep::NothingInFlight, |rules| self.start_write(rules))
    }

    fn start_write(&mut self, rules: String) -> SaveStep {
        self.write_slot = WriteSlot::Writing {
            rules: rules.clone(),
        };
        SaveStep::StartWrite(RulesWrite {
            path: self.path.clone(),
            rules,
        })
    }

    fn current_step(&self) -> SaveStep {
        match self.write_slot {
            WriteSlot::Idle => SaveStep::NothingInFlight,
            WriteSlot::Writing { .. } | WriteSlot::WritingThenNext { .. } => {
                SaveStep::WriteInFlight
            }
        }
    }

    fn newest_rules(&self) -> Option<&str> {
        match (&self.write_slot, &self.written) {
            (WriteSlot::WritingThenNext { next_rules, .. }, _) => Some(next_rules),
            (WriteSlot::Writing { rules }, _) | (WriteSlot::Idle, Written::Known(rules)) => {
                Some(rules)
            }
            (WriteSlot::Idle, Written::Unread | Written::Unreadable(_) | Written::Absent) => None,
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
    use std::{env, fs, io, path::Path, process};

    use super::{
        BackgroundEffect, HeatedWindow, NamespaceMatch, NiriLayout, NiriRulesFile, NiriStyling,
        RulesSaveError, RulesWrite, SaveStep, ShellBlurDepth, SottomanoRules, StoredRules,
        StyledSurface, SwitchHeat,
    };

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

    fn rules_file_after_first_write() -> NiriRulesFile {
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        let _ = rules.adopt(StoredRules::Absent, NiriStyling::default());
        rules.finish_write(Ok(()));
        rules
    }

    fn styled() -> NiriStyling {
        NiriStyling {
            blur_depth: ShellBlurDepth::WallpaperOnly,
            layout: NiriLayout {
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
        let read = NiriStyling::from(text.parse::<SottomanoRules>().unwrap());
        assert_eq!(read, written);
    }

    #[test]
    fn heated_windows_leave_the_look_unchanged_on_reading_back() {
        let written = styled();
        let text = SottomanoRules::styled(written, &heated_terminal())
            .to_kdl_v1()
            .unwrap();
        let read = NiriStyling::from(text.parse::<SottomanoRules>().unwrap());
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
    fn an_unchanged_heat_is_not_rewritten() {
        let mut rules = rules_file_after_first_write();
        assert!(matches!(
            rules.reheat(styled(), heated_terminal()),
            Ok(SaveStep::StartWrite(_))
        ));
        rules.finish_write(Ok(()));
        assert!(matches!(
            rules.reheat(styled(), heated_terminal()),
            Ok(SaveStep::NothingInFlight)
        ));
    }

    #[test]
    fn saving_before_the_file_is_read_is_refused() {
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        assert!(matches!(
            rules.save(styled()),
            Err(RulesSaveError::NotYetRead)
        ));
    }

    #[test]
    fn only_the_newest_rules_follow_a_write_in_flight() {
        let mut rules = rules_file_after_first_write();
        let first_look = styled();
        let second_look = NiriStyling {
            blur_depth: ShellBlurDepth::WindowsBehind,
            ..first_look
        };
        let third_look = NiriStyling {
            layout: NiriLayout {
                gaps_px: 9.0,
                ..first_look.layout
            },
            ..first_look
        };
        assert!(matches!(
            rules.save(first_look),
            Ok(SaveStep::StartWrite(_))
        ));
        assert!(matches!(
            rules.save(second_look),
            Ok(SaveStep::WriteInFlight)
        ));
        assert!(matches!(
            rules.save(third_look),
            Ok(SaveStep::WriteInFlight)
        ));
        let SaveStep::StartWrite(RulesWrite {
            rules: next_rules, ..
        }) = rules.finish_write(Ok(()))
        else {
            panic!("the waiting rules should start writing");
        };
        let read_back = NiriStyling::from(next_rules.parse::<SottomanoRules>().unwrap());
        assert_eq!(read_back, third_look);
        assert!(matches!(
            rules.finish_write(Ok(())),
            SaveStep::NothingInFlight
        ));
    }

    #[test]
    fn rules_matching_the_write_in_flight_start_nothing() {
        let mut rules = rules_file_after_first_write();
        assert!(matches!(rules.save(styled()), Ok(SaveStep::StartWrite(_))));
        assert!(matches!(rules.save(styled()), Ok(SaveStep::WriteInFlight)));
        assert!(matches!(
            rules.finish_write(Ok(())),
            SaveStep::NothingInFlight
        ));
    }

    #[test]
    fn a_failed_write_is_retried_by_an_identical_save() {
        let mut rules = rules_file_after_first_write();
        assert!(matches!(rules.save(styled()), Ok(SaveStep::StartWrite(_))));
        assert!(matches!(
            rules.finish_write(Err(io::ErrorKind::PermissionDenied)),
            SaveStep::NothingInFlight
        ));
        assert!(matches!(rules.save(styled()), Ok(SaveStep::StartWrite(_))));
    }

    #[tokio::test]
    async fn a_rules_write_replaces_the_file_without_a_staged_leftover() {
        let config_home = env::temp_dir().join(format!("sottomano-niri-rules-{}", process::id()));
        let mut rules = NiriRulesFile::in_config_home(&config_home);
        let adopted = rules.adopt(StoredRules::Absent, styled());
        let Ok(SaveStep::StartWrite(rules_write)) = adopted.saved else {
            panic!("an absent file should start a write");
        };
        rules_write.run().await.unwrap();
        let rules_directory = config_home.join("sottomano");
        let written_text = fs::read_to_string(rules_directory.join("niri.kdl")).unwrap();
        let read_back = NiriStyling::from(written_text.parse::<SottomanoRules>().unwrap());
        assert_eq!(read_back, styled());
        assert!(!rules_directory.join("niri.kdl.staged").exists());
        fs::remove_dir_all(&config_home).unwrap();
    }

    #[test]
    fn an_absent_file_keeps_the_defaults() {
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        let adopted = rules.adopt(StoredRules::Absent, NiriStyling::default());
        assert_eq!(adopted.styling, NiriStyling::default());
    }

    #[test]
    fn a_stored_look_is_adopted_over_the_current_one() {
        let text = SottomanoRules::styled(styled(), &SwitchHeat::Hidden)
            .to_kdl_v1()
            .unwrap();
        let mut rules = NiriRulesFile::in_config_home(Path::new("/nonexistent"));
        let adopted = rules.adopt(StoredRules::Text(text), NiriStyling::default());
        assert_eq!(adopted.styling, styled());
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
