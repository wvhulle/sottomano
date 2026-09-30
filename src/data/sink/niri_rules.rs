use std::{
    io,
    path::{Path, PathBuf},
};

use iced::Task;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use tokio::fs;

use crate::{
    state::{NiriStyling, PanelLayout, ShadowStyle},
    wayland_layers::{backdrop_shadow, surface::Position},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyledSurface {
    Panel,
    Sidebar,
    Pill,
}

impl StyledSurface {
    const ALL: [Self; 3] = [Self::Panel, Self::Sidebar, Self::Pill];

    pub fn namespace(self) -> &'static str {
        match self {
            Self::Panel => "vetrata-panel",
            Self::Sidebar => "vetrata-sidebar",
            Self::Pill => "vetrata-blur-pill",
        }
    }

    fn carries_chrome(self, layout: &PanelLayout) -> bool {
        match self {
            Self::Panel => !layout.disconnected_groups,
            Self::Sidebar => true,
            Self::Pill => layout.disconnected_groups,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NiriLook {
    pub corner_radius_px: f32,
    pub styling: NiriStyling,
}

impl Default for NiriLook {
    fn default() -> Self {
        Self {
            corner_radius_px: PanelLayout::default().corner_radius.top_left,
            styling: NiriStyling::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredRules {
    pub look: NiriLook,
    text: String,
}

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
        Task::future(async move {
            let text = fs::read_to_string(&path).await.unwrap_or_default();
            StoredRules {
                look: parse(&text),
                text,
            }
        })
    }

    pub fn adopt(&mut self, stored: &StoredRules) {
        self.written = Written::Known(stored.text.clone());
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

fn parse(text: &str) -> NiriLook {
    let fallback = NiriLook::default();
    let document = match KdlDocument::parse_v1(text) {
        Ok(document) => document,
        Err(e) => {
            tracing::warn!("niri rules: unreadable, starting from defaults: {e}");
            return fallback;
        }
    };
    let Some(rule) = rule_of(&document, StyledSurface::Sidebar) else {
        return fallback;
    };
    let shadow = rule.get("shadow").and_then(KdlNode::children);
    let effect = rule.get("background-effect").and_then(KdlNode::children);
    let default_shadow = fallback.styling.shadow;
    NiriLook {
        corner_radius_px: rule
            .get_arg("geometry-corner-radius")
            .and_then(number)
            .unwrap_or(fallback.corner_radius_px),
        styling: NiriStyling {
            blurred: effect
                .and_then(|effect| effect.get_arg("blur"))
                .and_then(KdlValue::as_bool)
                .unwrap_or(fallback.styling.blurred),
            xray: effect
                .and_then(|effect| effect.get_arg("xray"))
                .and_then(KdlValue::as_bool)
                .unwrap_or(fallback.styling.xray),
            shadow: shadow.map_or(default_shadow, |shadow| ShadowStyle {
                cast: shadow.get("on").is_some(),
                softness_px: shadow
                    .get_arg("softness")
                    .and_then(number)
                    .unwrap_or(default_shadow.softness_px),
                spread_px: shadow
                    .get_arg("spread")
                    .and_then(number)
                    .unwrap_or(default_shadow.spread_px),
                opacity: shadow
                    .get_arg("color")
                    .and_then(KdlValue::as_string)
                    .and_then(alpha_of)
                    .unwrap_or(default_shadow.opacity),
            }),
        },
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
            .map(|surface| layer_rule(surface, layout)),
    );
    document
        .nodes_mut()
        .extend(Position::BARS.into_iter().map(backdrop_rule));
    document.autoformat();
    document.ensure_v1();
    document.to_string()
}

fn layer_rule(surface: StyledSurface, layout: &PanelLayout) -> KdlNode {
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

fn backdrop_rule(position: Position) -> KdlNode {
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
                [KdlEntry::new_prop(
                    "namespace",
                    format!("^{}$", backdrop_shadow::namespace(position)),
                )],
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
    use super::{NiriLook, parse, render};
    use crate::state::{NiriStyling, PanelLayout, ShadowStyle};

    #[test]
    fn a_rendered_look_reads_back_unchanged() {
        let styling = NiriStyling {
            blurred: true,
            xray: true,
            shadow: ShadowStyle {
                cast: false,
                softness_px: 33.0,
                spread_px: 7.0,
                opacity: 0.4,
            },
        };
        let layout = PanelLayout {
            corner_radius: 9.0.into(),
            niri: styling,
            disconnected_groups: true,
            ..PanelLayout::default()
        };
        let look = parse(&render(&layout));
        assert!((look.corner_radius_px - 9.0).abs() < f32::EPSILON);
        assert_eq!(look.styling.blurred, styling.blurred);
        assert_eq!(look.styling.xray, styling.xray);
        assert_eq!(look.styling.shadow.cast, styling.shadow.cast);
        assert!((look.styling.shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((look.styling.shadow.spread_px - 7.0).abs() < f32::EPSILON);
        assert!((look.styling.shadow.opacity - 0.4).abs() < 0.01);
    }

    #[test]
    fn a_missing_file_reads_as_the_default_look() {
        assert_eq!(parse(""), NiriLook::default());
    }

    fn rule_for<'a>(rules: &'a str, namespace: &str) -> &'a str {
        let start = rules
            .find(&format!("\"^{namespace}$\""))
            .expect("rule for namespace");
        let end = rules[start..]
            .find("layer-rule")
            .map_or(rules.len(), |offset| start + offset);
        &rules[start..end]
    }

    fn blurred(disconnected_groups: bool) -> PanelLayout {
        PanelLayout {
            niri: NiriStyling {
                blurred: true,
                ..NiriStyling::default()
            },
            disconnected_groups,
            ..PanelLayout::default()
        }
    }

    #[test]
    fn rules_parse_as_kdl_v1() {
        let rules = render(&PanelLayout::default());
        kdl::KdlDocument::parse_v1(&rules).expect("valid KDL v1");
    }

    #[test]
    fn joined_groups_style_the_strip_and_not_the_pills() {
        let rules = render(&blurred(false));
        assert!(rule_for(&rules, "vetrata-panel").contains("blur true"));
        assert!(rule_for(&rules, "vetrata-blur-pill").contains("blur false"));
    }

    #[test]
    fn separate_pills_move_the_chrome_off_the_strip() {
        let rules = render(&blurred(true));
        assert!(rule_for(&rules, "vetrata-panel").contains("blur false"));
        assert!(rule_for(&rules, "vetrata-blur-pill").contains("blur true"));
        assert!(rule_for(&rules, "vetrata-sidebar").contains("blur true"));
    }

    #[test]
    fn radius_and_shadow_opacity_reach_the_rules() {
        let layout = PanelLayout {
            corner_radius: 5.5.into(),
            niri: NiriStyling {
                shadow: ShadowStyle {
                    opacity: 0.5,
                    ..ShadowStyle::default()
                },
                ..NiriStyling::default()
            },
            ..PanelLayout::default()
        };
        let rules = render(&layout);
        assert!(rules.contains("geometry-corner-radius 5.5"));
        assert!(rules.contains("\"#00000080\""));
    }

    #[test]
    fn xray_follows_the_switch() {
        let off = render(&PanelLayout::default());
        assert_eq!(off.matches("xray false").count(), 3);
        let on = render(&PanelLayout {
            niri: NiriStyling {
                xray: true,
                ..NiriStyling::default()
            },
            ..PanelLayout::default()
        });
        assert_eq!(on.matches("xray true").count(), 3);
    }
}
