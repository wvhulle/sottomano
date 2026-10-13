pub mod spacing;

use serde::{Deserialize, Serialize};

use crate::config::layout::spacing::{Length, Spacing};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + Length")
)]
pub struct LayoutConfig<L = f32> {
    pub panel: PanelLayout<L>,
    pub overlay: OverlayLayout<L>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + Length")
)]
pub struct PanelLayout<L = f32> {
    pub height: L,
    pub group_padding: L,
    pub widget_spacing: L,
    pub group_spacing: L,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + Length")
)]
pub struct OverlayLayout<L = f32> {
    pub panel_gap: L,
    pub content_inset: L,
    pub row_spacing: L,
}

impl<L: Length> Default for LayoutConfig<L> {
    fn default() -> Self {
        Self {
            panel: PanelLayout::default(),
            overlay: OverlayLayout::default(),
        }
    }
}

impl<L: Length> Default for PanelLayout<L> {
    fn default() -> Self {
        Self {
            height: L::px(24.0),
            group_padding: L::px(5.0),
            widget_spacing: L::px(4.0),
            group_spacing: L::px(11.0),
        }
    }
}

impl<L: Length> Default for OverlayLayout<L> {
    fn default() -> Self {
        Self {
            panel_gap: L::px(14.0),
            content_inset: L::px(10.0),
            row_spacing: L::px(4.0),
        }
    }
}

impl LayoutConfig<Spacing> {
    pub fn resolve(self, em_px: f32) -> LayoutConfig {
        LayoutConfig {
            panel: self.panel.resolve(em_px),
            overlay: self.overlay.resolve(em_px),
        }
    }
}

impl PanelLayout<Spacing> {
    pub fn resolve(self, em_px: f32) -> PanelLayout {
        PanelLayout {
            height: self.height.resolve(em_px),
            group_padding: self.group_padding.resolve(em_px),
            widget_spacing: self.widget_spacing.resolve(em_px),
            group_spacing: self.group_spacing.resolve(em_px),
        }
    }
}

impl OverlayLayout<Spacing> {
    pub fn resolve(self, em_px: f32) -> OverlayLayout {
        OverlayLayout {
            panel_gap: self.panel_gap.resolve(em_px),
            content_inset: self.content_inset.resolve(em_px),
            row_spacing: self.row_spacing.resolve(em_px),
        }
    }
}
