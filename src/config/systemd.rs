use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct SystemdConfig {
    pub slow_job_secs: u64,
    pub slow_boot_unit_secs: u64,
    pub slow_shutdown_stop_secs: u64,
}

impl Default for SystemdConfig {
    fn default() -> Self {
        Self {
            slow_job_secs: 10,
            slow_boot_unit_secs: 5,
            slow_shutdown_stop_secs: 5,
        }
    }
}

impl SystemdConfig {
    pub fn slow_job(&self) -> Duration {
        Duration::from_secs(self.slow_job_secs)
    }

    pub fn slow_boot_unit(&self) -> Duration {
        Duration::from_secs(self.slow_boot_unit_secs)
    }

    pub fn slow_shutdown_stop(&self) -> Duration {
        Duration::from_secs(self.slow_shutdown_stop_secs)
    }
}
