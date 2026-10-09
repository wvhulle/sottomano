use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::data::severity::Severity;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct ToastLifetimeConfig {
    pub low_severity_timeout_ms: u64,
    pub normal_severity_timeout_ms: u64,
    pub critical_severity_timeout_ms: u64,
}

impl Default for ToastLifetimeConfig {
    fn default() -> Self {
        Self {
            low_severity_timeout_ms: 8_000,
            normal_severity_timeout_ms: 12_000,
            critical_severity_timeout_ms: 0,
        }
    }
}

impl ToastLifetimeConfig {
    pub fn default_timeout_for(&self, severity: Severity) -> Option<Duration> {
        let configured_millis = match severity {
            Severity::Low => self.low_severity_timeout_ms,
            Severity::Normal | Severity::Warning => self.normal_severity_timeout_ms,
            Severity::Critical => self.critical_severity_timeout_ms,
        };
        (configured_millis > 0).then(|| Duration::from_millis(configured_millis))
    }
}
