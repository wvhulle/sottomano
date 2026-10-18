pub mod spacing;

use serde::{Deserialize, Serialize};

use crate::layout::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct LayoutConfig<L = f32> {
    pub panel: PanelLayout<L>,
    pub overlay: OverlayLayout<L>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
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
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct OverlayLayout<L = f32> {
    pub panel_gap: L,
    pub content_inset: L,
    pub row_spacing: L,
}

impl<L: From<f32>> Default for LayoutConfig<L> {
    fn default() -> Self {
        Self {
            panel: PanelLayout::default(),
            overlay: OverlayLayout::default(),
        }
    }
}

impl<L: From<f32>> Default for PanelLayout<L> {
    fn default() -> Self {
        Self {
            height: L::from(24.0),
            group_padding: L::from(5.0),
            widget_spacing: L::from(4.0),
            group_spacing: L::from(11.0),
        }
    }
}

impl<L: From<f32>> Default for OverlayLayout<L> {
    fn default() -> Self {
        Self {
            panel_gap: L::from(14.0),
            content_inset: L::from(10.0),
            row_spacing: L::from(4.0),
        }
    }
}

impl LayoutConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> LayoutConfig {
        LayoutConfig {
            panel: self.panel.resolve(em_px),
            overlay: self.overlay.resolve(em_px),
        }
    }
}

impl PanelLayout<Spacing> {
    #[must_use]
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
    #[must_use]
    pub fn resolve(self, em_px: f32) -> OverlayLayout {
        OverlayLayout {
            panel_gap: self.panel_gap.resolve(em_px),
            content_inset: self.content_inset.resolve(em_px),
            row_spacing: self.row_spacing.resolve(em_px),
        }
    }
}
