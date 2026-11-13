use serde::{Deserialize, Serialize};

use crate::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct SpacingTokens<L = f32> {
    pub item_gap: L,
    pub surface_inset: L,
    pub group_gap: L,
}

impl<L: From<f32>> Default for SpacingTokens<L> {
    fn default() -> Self {
        Self {
            item_gap: L::from(6.0),
            surface_inset: L::from(10.0),
            group_gap: L::from(14.0),
        }
    }
}

impl SpacingTokens<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> SpacingTokens {
        SpacingTokens {
            item_gap: self.item_gap.resolve(em_px),
            surface_inset: self.surface_inset.resolve(em_px),
            group_gap: self.group_gap.resolve(em_px),
        }
    }
}
