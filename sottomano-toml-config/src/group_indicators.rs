use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GroupIndicatorsConfig {
    #[serde(with = "crate::friendly_duration")]
    pub slider_expansion_duration: Duration,
}

impl Default for GroupIndicatorsConfig {
    fn default() -> Self {
        Self {
            slider_expansion_duration: Duration::from_millis(180),
        }
    }
}
