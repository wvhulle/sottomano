use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OverlaySurfacesConfig {
    #[serde(with = "humantime_serde")]
    pub surface_morph_duration: Duration,
}

impl Default for OverlaySurfacesConfig {
    fn default() -> Self {
        Self {
            surface_morph_duration: Duration::from_millis(160),
        }
    }
}
