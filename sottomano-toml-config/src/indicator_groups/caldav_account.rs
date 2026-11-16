use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaldavAccountConfig {
    pub server_url: String,
    pub user: String,
    pub upcoming_days: u32,
    #[serde(with = "humantime_serde")]
    pub change_poll_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub push_safety_recheck_interval: Duration,
    #[serde(with = "humantime_serde")]
    pub subscription_download_interval: Duration,
}

impl Default for CaldavAccountConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            user: String::new(),
            upcoming_days: 7,
            change_poll_interval: Duration::from_mins(1),
            push_safety_recheck_interval: Duration::from_mins(15),
            subscription_download_interval: Duration::from_mins(15),
        }
    }
}

impl CaldavAccountConfig {
    #[must_use]
    pub fn configured_server_url(&self) -> Option<&str> {
        filled(&self.server_url)
    }

    #[must_use]
    pub fn configured_user(&self) -> Option<&str> {
        filled(&self.user)
    }
}

fn filled(value: &str) -> Option<&str> {
    Some(value.trim()).filter(|trimmed| !trimmed.is_empty())
}
