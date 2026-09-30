use serde::Deserialize;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chrome {
    pub popup_gap_px: f32,
    pub outer_height_px: f32,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            popup_gap_px: 6.0,
            outer_height_px: 24.0,
        }
    }
}
