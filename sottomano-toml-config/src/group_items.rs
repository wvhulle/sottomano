use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GroupItemsConfig {
    #[serde(with = "humantime_serde")]
    pub slider_expansion_duration: Duration,
}

impl Default for GroupItemsConfig {
    fn default() -> Self {
        Self {
            slider_expansion_duration: Duration::from_millis(90),
        }
    }
}
