use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct OverlaySurfacesConfig<L = f32> {
    pub content_inset: L,
    pub row_gap: L,
    pub column_gap: L,
    pub section_gap: L,
    pub satellite_gap: L,
    #[serde(with = "humantime_serde")]
    pub surface_morph_duration: Duration,
}

impl<L: From<f32>> Default for OverlaySurfacesConfig<L> {
    fn default() -> Self {
        Self {
            content_inset: L::from(10.0),
            row_gap: L::from(4.0),
            column_gap: L::from(14.0),
            section_gap: L::from(16.0),
            satellite_gap: L::from(14.0),
            surface_morph_duration: Duration::from_millis(160),
        }
    }
}

impl OverlaySurfacesConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> OverlaySurfacesConfig {
        OverlaySurfacesConfig {
            content_inset: self.content_inset.resolve(em_px),
            row_gap: self.row_gap.resolve(em_px),
            column_gap: self.column_gap.resolve(em_px),
            section_gap: self.section_gap.resolve(em_px),
            satellite_gap: self.satellite_gap.resolve(em_px),
            surface_morph_duration: self.surface_morph_duration,
        }
    }
}
