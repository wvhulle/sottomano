use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowListConfig {
    pub audio_indicator: bool,
    pub progress: bool,
    pub count_badge: bool,
}

impl Default for WindowListConfig {
    fn default() -> Self {
        Self {
            audio_indicator: true,
            progress: true,
            count_badge: true,
        }
    }
}
