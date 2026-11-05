use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct SystemdManagerConfig {
    #[serde(with = "humantime_serde")]
    pub slow_job: Duration,
    #[serde(with = "humantime_serde")]
    pub slow_boot_unit: Duration,
    #[serde(with = "humantime_serde")]
    pub slow_shutdown_stop: Duration,
    pub slower_boot_warning_percent: u32,
    pub slower_boot_critical_percent: u32,
}

impl Default for SystemdManagerConfig {
    fn default() -> Self {
        Self {
            slow_job: Duration::from_secs(10),
            slow_boot_unit: Duration::from_secs(5),
            slow_shutdown_stop: Duration::from_secs(5),
            slower_boot_warning_percent: 25,
            slower_boot_critical_percent: 60,
        }
    }
}
