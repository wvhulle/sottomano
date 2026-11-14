use serde::{Deserialize, Serialize};

use crate::severity::Severity;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GroupMenusConfig {
    pub alert_promotion_severity: Severity,
    pub scroll_fade_rows: f32,
}

impl Default for GroupMenusConfig {
    fn default() -> Self {
        Self {
            alert_promotion_severity: Severity::Critical,
            scroll_fade_rows: 1.5,
        }
    }
}
