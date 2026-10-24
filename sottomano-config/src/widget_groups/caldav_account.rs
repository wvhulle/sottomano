use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaldavAccountConfig {
    pub server_url: String,
    pub user: String,
    pub web_url: String,
    pub upcoming_days: u32,
}

impl Default for CaldavAccountConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            user: String::new(),
            web_url: String::new(),
            upcoming_days: 7,
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

    #[must_use]
    pub fn configured_web_url(&self) -> Option<&str> {
        filled(&self.web_url)
    }
}

fn filled(value: &str) -> Option<&str> {
    Some(value.trim()).filter(|trimmed| !trimmed.is_empty())
}
