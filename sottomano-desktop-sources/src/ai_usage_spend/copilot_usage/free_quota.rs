use chrono::{DateTime, Utc};
use futures::Stream;

use super::{CopilotAccount, CopilotPlan, Error, plan_polls, user_info};
use crate::{SnapshotDelivery, SnapshotSource};

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotFree {
    pub plan: Option<String>,
    pub chat: i64,
    pub completions: i64,
    pub reset: Option<DateTime<Utc>>,
}

impl SnapshotSource for CopilotFree {
    type Error = Error;
    type StreamInputs = CopilotAccount;

    const NAME: &'static str = "Copilot free quota";

    fn delivery_stream(
        inputs: CopilotAccount,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        plan_polls(inputs, CopilotPlan::Free, Self::poll)
    }
}

impl CopilotFree {
    async fn poll(key: CopilotAccount) -> Result<Self, Error> {
        let info = user_info(&key.gh_auth_token).await?;
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

#[cfg(test)]
mod tests {
    use super::{super::UserInfo, *};

    const FREE_USER_INFO: &str = r#"{
        "access_type_sku": "free",
        "limited_user_quotas": { "chat": 12, "completions": 900 },
        "limited_user_reset_date": 1788220800
    }"#;

    #[test]
    fn limited_quotas_mark_a_free_account() {
        let info: UserInfo = serde_json::from_str(FREE_USER_INFO).unwrap();
        assert_eq!(info.plan_kind(), CopilotPlan::Free);
        let quotas = info.limited_user_quotas.unwrap();
        assert_eq!(quotas.chat, 12);
        assert_eq!(quotas.completions, 900);
    }
}
