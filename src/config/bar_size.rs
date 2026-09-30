use serde::Deserialize;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BarSize {
    pub height_px: f32,
    pub popup_gap_px: f32,
}

impl Default for BarSize {
    fn default() -> Self {
        Self {
            height_px: 24.0,
            popup_gap_px: 6.0,
        }
    }
}
