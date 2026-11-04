use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OverlaySurfacesConfig {
    #[serde(with = "humantime_serde")]
    pub surface_morph_duration: Duration,
    #[serde(with = "humantime_serde")]
    pub typing_idle_delay: Duration,
    pub typing_background_opacity: f32,
}

impl Default for OverlaySurfacesConfig {
    fn default() -> Self {
        Self {
            surface_morph_duration: Duration::from_millis(160),
            typing_idle_delay: Duration::from_millis(750),
            typing_background_opacity: 0.3,
        }
    }
}
