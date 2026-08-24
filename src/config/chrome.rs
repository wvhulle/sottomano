use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chrome {
    pub popup_radius_px: u16,
    pub popup_gap_px: f32,
    pub outer_height_px: f32,
    pub shadow: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            popup_radius_px: 8,
            popup_gap_px: 6.0,
            outer_height_px: 24.0,
            shadow: true,
        }
    }
}
