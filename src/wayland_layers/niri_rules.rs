use std::{
    io,
    path::{Path, PathBuf},
};

use iced::Task;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use tokio::fs;

use crate::{
    state::{PanelLayout, ShadowStyle},
    wayland_layers::surface::Position,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    Sidebar,
    Pill,
    Backdrop(Position),
}

impl StyledSurface {
    const ALL: [Self; 5] = [
        Self::Panel,
        Self::Sidebar,
        Self::Pill,
        Self::Backdrop(Position::Top),
        Self::Backdrop(Position::Bottom),
    ];

    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "vetrata-panel",
            Self::Sidebar => "vetrata-sidebar",
            Self::Pill => "vetrata-pill",
            Self::Backdrop(Position::Top) => "vetrata-backdrop-top",
            Self::Backdrop(Position::Bottom) => "vetrata-backdrop-bottom",
            Self::Backdrop(Position::Left) => "vetrata-backdrop-left",
        }
    }

    fn carries_chrome(self, layout: &PanelLayout) -> bool {
        match self {
            Self::Panel => !layout.disconnected_groups,
            Self::Sidebar => true,
            Self::Pill => layout.disconnected_groups,
            Self::Backdrop(_) => false,
        }
    }

    fn rule(self, layout: &PanelLayout) -> KdlNode {
        match self {
            Self::Backdrop(position) => backdrop_rule(self, position),
            Self::Panel | Self::Sidebar | Self::Pill => chrome_rule(self, layout),
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
    let Some(rule) = rule_of(&document, StyledSurface::Sidebar) else {
        return;
    };
    let arg = |block: &str, name: &str| {
        rule.get(block)
            .and_then(KdlNode::children)
            .and_then(|children| children.get_arg(name))
    };
    let niri = &mut layout.niri;
    let shadow = &mut niri.shadow;
    set(
        &mut niri.blurred,
        arg("background-effect", "blur").and_then(KdlValue::as_bool),
    );
    set(
        &mut niri.xray,
        arg("background-effect", "xray").and_then(KdlValue::as_bool),
    );
    set(
        &mut shadow.softness_px,
        arg("shadow", "softness").and_then(number),
    );
    set(
        &mut shadow.spread_px,
        arg("shadow", "spread").and_then(number),
    );
    set(
        &mut shadow.opacity,
        arg("shadow", "color")
            .and_then(KdlValue::as_string)
            .and_then(alpha_of),
    );
    if let Some(block) = rule.get("shadow").and_then(KdlNode::children) {
        shadow.cast = block.get("on").is_some();
    }
    if let Some(radius) = rule.get_arg("geometry-corner-radius").and_then(number) {
        layout.corner_radius = radius.into();
    }
}

fn set<T>(field: &mut T, read: Option<T>) {
    if let Some(value) = read {
        *field = value;
    }
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

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn number(value: &KdlValue) -> Option<f32> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|integer| integer as f64))
        .map(|float| float as f32)
}

fn alpha_of(color: &str) -> Option<f32> {
    let hex = color.strip_prefix('#')?;
    let alpha = hex.get(6..8)?;
    u8::from_str_radix(alpha, 16)
        .ok()
        .map(|alpha| f32::from(alpha) / 255.0)
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

fn chrome_rule(surface: StyledSurface, layout: &PanelLayout) -> KdlNode {
    let carries_chrome = surface.carries_chrome(layout);
    block(
        "layer-rule",
        [
            node(
                "match",
                [KdlEntry::new_prop("namespace", namespace_pattern(surface))],
            ),
            leaf(
                "geometry-corner-radius",
                f64::from(layout.corner_radius.top_left),
            ),
            shadow(layout.niri.shadow, carries_chrome),
            block(
                "background-effect",
                [
                    leaf("blur", layout.niri.blurred && carries_chrome),
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

fn shadow(style: ShadowStyle, carries_chrome: bool) -> KdlNode {
    let switch = if style.cast && carries_chrome {
        "on"
    } else {
        "off"
    };
    block(
        "shadow",
        [
            KdlNode::new(switch),
            leaf("softness", f64::from(style.softness_px)),
            leaf("spread", f64::from(style.spread_px)),
            leaf("color", shadow_color(style.opacity)),
        ],
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn shadow_color(opacity: f32) -> String {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#000000{alpha:02x}")
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
    use crate::state::{NiriStyling, PanelLayout, ShadowStyle};

    fn styled(disconnected_groups: bool) -> PanelLayout {
        PanelLayout {
            corner_radius: 5.5.into(),
            niri: NiriStyling {
                blurred: true,
                xray: true,
                shadow: ShadowStyle {
                    cast: false,
                    softness_px: 33.0,
                    spread_px: 7.0,
                    opacity: 0.4,
                },
            },
            disconnected_groups,
            ..PanelLayout::default()
        }
    }

    #[test]
    fn a_rendered_look_reads_back_unchanged() {
        let written = styled(true);
        let mut read = PanelLayout::default();
        apply(&render(&written), &mut read);
        assert_eq!(read.corner_radius, written.corner_radius);
        assert_eq!(read.niri, written.niri);
    }

    #[test]
    fn an_empty_file_keeps_the_defaults() {
        let mut read = PanelLayout::default();
        apply("", &mut read);
        assert_eq!(read.niri, PanelLayout::default().niri);
    }

    #[test]
    fn the_blur_follows_the_grouping() {
        let blur_of = |disconnected, namespace: &str| {
            let rules = render(&styled(disconnected));
            let start = rules.find(&format!("\"^{namespace}$\"")).expect("rule");
            rules[start..]
                .split("layer-rule")
                .next()
                .is_some_and(|rule| rule.contains("blur true"))
        };
        assert!(blur_of(false, "vetrata-panel"));
        assert!(!blur_of(false, "vetrata-pill"));
        assert!(!blur_of(true, "vetrata-panel"));
        assert!(blur_of(true, "vetrata-pill"));
    }
}
