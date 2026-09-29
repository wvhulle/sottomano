use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Utc};

use super::{Error, USER_POLL_INTERVAL, UserInfo, user_info};
use crate::data::source::Source;

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotFree {
    pub plan: String,
    pub chat: i64,
    pub completions: i64,
    pub reset: Option<DateTime<Utc>>,
}

impl Source for CopilotFree {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "copilot-usage-free";
    const INTERVAL: Duration = USER_POLL_INTERVAL;

    async fn available(key: &PathBuf) -> bool {
        user_info(key).await.is_ok_and(|info| info.free_plan())
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let info = user_info(&key).await?;
        let quotas = info.limited_user_quotas.as_ref().ok_or(Error::NoQuota)?;
        Ok(CopilotFree {
            plan: info.plan(),
            chat: quotas.chat,
            completions: quotas.completions,
            reset: info
                .limited_user_reset_date
                .filter(|epoch| *epoch > 0)
                .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        })
    }
}

impl UserInfo {
    fn free_plan(&self) -> bool {
        self.limited_user_quotas.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FREE_USER_INFO: &str = r#"{
        "access_type_sku": "free",
        "limited_user_quotas": { "chat": 12, "completions": 900 },
        "limited_user_reset_date": 1788220800
    }"#;

    #[test]
    fn limited_quotas_mark_a_free_account() {
        let info: UserInfo = serde_json::from_str(FREE_USER_INFO).unwrap();
        assert!(info.free_plan());
        let quotas = info.limited_user_quotas.unwrap();
        assert_eq!(quotas.chat, 12);
        assert_eq!(quotas.completions, 900);
    }
}
