use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WidgetGroupLayoutConfig {
    #[serde(with = "humantime_serde")]
    pub expansion_duration: Duration,
}

impl Default for WidgetGroupLayoutConfig {
    fn default() -> Self {
        Self {
            expansion_duration: Duration::from_millis(90),
        }
    }
}
