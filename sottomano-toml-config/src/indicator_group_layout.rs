use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct IndicatorGroupLayoutConfig {
    #[serde(with = "crate::friendly_duration")]
    pub expansion_duration: Duration,
}

impl Default for IndicatorGroupLayoutConfig {
    fn default() -> Self {
        Self {
            expansion_duration: Duration::from_millis(180),
        }
    }
}
