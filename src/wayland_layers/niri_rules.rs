use std::{
    io,
    path::{Path, PathBuf},
};

use iced::Task;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use tokio::fs;

use crate::{state::PanelLayout, wayland_layers::surface::Position};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    Backdrop(Position),
}

impl StyledSurface {
    const ALL: [Self; 3] = [
        Self::Panel,
        Self::Backdrop(Position::Top),
        Self::Backdrop(Position::Bottom),
    ];

    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "vetrata-panel",
            Self::Backdrop(Position::Top) => "vetrata-backdrop-top",
            Self::Backdrop(Position::Bottom) => "vetrata-backdrop-bottom",
            Self::Backdrop(Position::Left) => "vetrata-backdrop-left",
        }
    }

    fn rule(self, layout: &PanelLayout) -> KdlNode {
        match self {
            Self::Backdrop(position) => backdrop_rule(self, position),
            Self::Panel => panel_rule(self, layout),
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
}

impl NiriRulesFile {
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: config_home.join("vetrata").join("niri.kdl"),
            written: Written::Unread,
        }
    }

    pub fn read(&self) -> Task<StoredRules> {
        let path = self.path.clone();
        Task::future(
            async move { StoredRules(fs::read_to_string(&path).await.unwrap_or_default()) },
        )
    }

    pub fn adopt(&mut self, stored: StoredRules, layout: &mut PanelLayout) {
        apply(&stored.0, layout);
        self.written = Written::Known(stored.0);
    }

    pub fn write(&mut self, layout: &PanelLayout) -> Task<bool> {
        let Written::Known(last) = &self.written else {
            return Task::done(false);
        };
        let rules = render(layout);
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

fn apply(text: &str, layout: &mut PanelLayout) {
    let document = match KdlDocument::parse_v1(text) {
        Ok(document) => document,
        Err(e) => {
            tracing::warn!("niri rules: unreadable, keeping defaults: {e}");
            return;
        }
    };
    let Some(effect) = rule_of(&document, StyledSurface::Panel)
        .and_then(|rule| rule.get("background-effect"))
        .and_then(KdlNode::children)
    else {
        return;
    };
    let switch = |name: &str| effect.get_arg(name).and_then(KdlValue::as_bool);
    let niri = &mut layout.niri;
    niri.blurred = switch("blur").unwrap_or(niri.blurred);
    niri.xray = switch("xray").unwrap_or(niri.xray);
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

pub fn render(layout: &PanelLayout) -> String {
    let mut document = KdlDocument::new();
    document.nodes_mut().extend(
        StyledSurface::ALL
            .into_iter()
            .map(|surface| surface.rule(layout)),
    );
    document.autoformat();
    document.ensure_v1();
    document.to_string()
}

fn panel_rule(surface: StyledSurface, layout: &PanelLayout) -> KdlNode {
    block(
        "layer-rule",
        [
            node(
                "match",
                [KdlEntry::new_prop("namespace", namespace_pattern(surface))],
            ),
            block(
                "background-effect",
                [
                    leaf("blur", layout.niri.blurred),
                    leaf("xray", layout.niri.xray),
                ],
            ),
        ],
    )
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
    use super::{apply, render};
    use crate::state::{NiriStyling, PanelLayout};

    fn styled() -> PanelLayout {
        PanelLayout {
            niri: NiriStyling {
                blurred: true,
                xray: true,
            },
            ..PanelLayout::default()
        }
    }

    #[test]
    fn a_rendered_look_reads_back_unchanged() {
        let written = styled();
        let mut read = PanelLayout::default();
        apply(&render(&written), &mut read);
        assert_eq!(read.niri, written.niri);
    }

    #[test]
    fn an_empty_file_keeps_the_defaults() {
        let mut read = PanelLayout::default();
        apply("", &mut read);
        assert_eq!(read.niri, PanelLayout::default().niri);
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
