use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScrollingConfig {
    pub wheel_line_px: f32,
    #[serde(with = "humantime_serde")]
    pub wheel_easing: Duration,
}

impl Default for ScrollingConfig {
    fn default() -> Self {
        Self {
            wheel_line_px: 60.0,
            wheel_easing: Duration::from_millis(140),
        }
    }
}
