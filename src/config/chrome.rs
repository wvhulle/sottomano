use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Chrome {
    pub corner_radius_px: u16,
    pub outer_height_px: f32,
    pub shadow: bool,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            corner_radius_px: 8,
            outer_height_px: 25.0,
            shadow: true,
        }
    }
}
