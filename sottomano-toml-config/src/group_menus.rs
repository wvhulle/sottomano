use serde::{Deserialize, Serialize};

use crate::severity::Severity;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GroupMenusConfig {
    pub alert_promotion_severity: Severity,
}

impl Default for GroupMenusConfig {
    fn default() -> Self {
        Self {
            alert_promotion_severity: Severity::Warning,
        }
    }
}
