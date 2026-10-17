use std::{
    io,
    path::{Path, PathBuf},
};

use iced::Task;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
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

    fn rule(self, styling: &NiriStyling) -> KdlNode {
        match self {
            Self::Backdrop(position) => backdrop_rule(self, position),
            Self::Panel | Self::Tooltip => background_effect_rule(self, styling),
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
        apply(&stored.0, &mut self.styling);
        self.written = Written::Known(stored.0);
        self.write()
    }

    fn write(&mut self) -> Task<bool> {
        let Written::Known(last) = &self.written else {
            return Task::done(false);
        };
        let rules = render(&self.styling);
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

fn apply(text: &str, styling: &mut NiriStyling) {
    let document = match parse_niri_kdl(text) {
        Ok(document) => document,
        Err(e) => {
            tracing::warn!("niri rules: unreadable, keeping defaults: {e}");
            return;
        }
    };
    apply_background_effect(&document, styling);
    apply_window_layout(&document, &mut styling.layout);
}

fn apply_background_effect(document: &KdlDocument, niri: &mut NiriStyling) {
    let Some(effect) = rule_of(document, StyledSurface::Panel)
        .and_then(|rule| rule.get("background-effect"))
        .and_then(KdlNode::children)
    else {
        return;
    };
    niri.xray = effect
        .get_arg("xray")
        .and_then(KdlValue::as_bool)
        .unwrap_or(niri.xray);
}

fn apply_window_layout(document: &KdlDocument, windows: &mut NiriLayout) {
    let setting = |section: &str, name: &str| {
        document
            .get(section)
            .and_then(KdlNode::children)
            .and_then(|children| children.get_arg(name))
            .and_then(number)
    };
    windows.gaps_px = setting("layout", "gaps").unwrap_or(windows.gaps_px);
    windows.window_corner_radius_px =
        setting("window-rule", "geometry-corner-radius").unwrap_or(windows.window_corner_radius_px);
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn number(value: &KdlValue) -> Option<f32> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|integer| integer as f64))
        .map(|float| float as f32)
}

fn rule_of(document: &KdlDocument, surface: StyledSurface) -> Option<&KdlDocument> {
    let pattern = namespace_pattern(surface);
    document
        .nodes()
        .iter()
        .filter(|node| node.name().value() == "layer-rule")
        .filter_map(KdlNode::children)
        .find(|rule| {
            rule.get("match")
                .and_then(|matcher| matcher.get("namespace"))
                .and_then(KdlValue::as_string)
                == Some(pattern.as_str())
        })
}

fn namespace_pattern(surface: StyledSurface) -> String {
    format!("^{}$", surface.namespace())
}

async fn replace(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).await?;
    }
    let staged = path.with_extension("kdl.staged");
    fs::write(&staged, contents).await?;
    fs::rename(&staged, path).await
}

fn render(styling: &NiriStyling) -> String {
    let mut document = KdlDocument::new();
    document.nodes_mut().extend(
        StyledSurface::ALL
            .into_iter()
            .map(|surface| surface.rule(styling))
            .chain(window_layout(styling.layout)),
    );
    document.autoformat();
    document.ensure_v1();
    document.to_string()
}

fn background_effect_rule(surface: StyledSurface, styling: &NiriStyling) -> KdlNode {
    block(
        "layer-rule",
        [
            node(
                "match",
                [KdlEntry::new_prop("namespace", namespace_pattern(surface))],
            ),
            block(
                "background-effect",
                [leaf("blur", true), leaf("xray", styling.xray)],
            ),
        ],
    )
}

fn window_layout(windows: NiriLayout) -> [KdlNode; 2] {
    [
        block("layout", [leaf("gaps", f64::from(windows.gaps_px))]),
        block(
            "window-rule",
            [
                leaf(
                    "geometry-corner-radius",
                    f64::from(windows.window_corner_radius_px),
                ),
                leaf("clip-to-geometry", true),
            ],
        ),
    ]
}

fn backdrop_rule(surface: StyledSurface, position: Position) -> KdlNode {
    let lift: i128 = match position {
        Position::Top => -3,
        Position::Bottom => 3,
        Position::Left => 0,
    };
    block(
        "layer-rule",
        [
            node(
                "match",
                [KdlEntry::new_prop("namespace", namespace_pattern(surface))],
            ),
            block(
                "shadow",
                [
                    KdlNode::new("on"),
                    leaf("softness", 27_i128),
                    leaf("spread", 25_i128),
                    leaf("color", "#00000060"),
                    node(
                        "offset",
                        [
                            KdlEntry::new_prop("x", 0_i128),
                            KdlEntry::new_prop("y", lift),
                        ],
                    ),
                ],
            ),
        ],
    )
}

fn leaf(name: &str, value: impl Into<KdlValue>) -> KdlNode {
    node(name, [KdlEntry::new(value)])
}

fn node(name: &str, entries: impl IntoIterator<Item = KdlEntry>) -> KdlNode {
    let mut node = KdlNode::new(name);
    node.entries_mut().extend(entries);
    node
}

fn block(name: &str, children: impl IntoIterator<Item = KdlNode>) -> KdlNode {
    let mut node = KdlNode::new(name);
    node.ensure_children().nodes_mut().extend(children);
    node
}

#[cfg(test)]
mod tests {
    use super::{NiriLayout, NiriStyling, apply, render};

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
        let mut read = NiriStyling::default();
        apply(&render(&written), &mut read);
        assert_eq!(read, written);
    }

    #[test]
    fn an_empty_file_keeps_the_defaults() {
        let mut read = NiriStyling::default();
        apply("", &mut read);
        assert_eq!(read, NiriStyling::default());
    }

    #[test]
    fn the_panel_leaves_shadows_and_corners_to_vetrata() {
        let rules = render(&styled());
        let start = rules.find("\"^vetrata-panel$\"").expect("panel rule");
        let panel_rule = rules[start..]
            .split("layer-rule")
            .next()
            .unwrap_or_default();
        assert!(panel_rule.contains("blur true"));
        assert!(!panel_rule.contains("shadow") && !panel_rule.contains("corner"));
    }
}
