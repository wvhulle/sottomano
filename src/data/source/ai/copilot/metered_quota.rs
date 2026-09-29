use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Utc};

use super::{Error, QuotaSnapshot, USER_POLL_INTERVAL, UserInfo, quota_reset, user_info};
use crate::data::source::Source;

#[derive(Debug, Clone, PartialEq)]
pub struct CopilotMetered {
    pub plan: String,
    pub quota: String,
    pub used: f64,
    pub limit: i64,
    pub remaining: f64,
    pub percent_used: f64,
    pub reset: Option<DateTime<Utc>>,
}

impl Source for CopilotMetered {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "copilot-usage-metered";
    const INTERVAL: Duration = USER_POLL_INTERVAL;

    async fn available(key: &PathBuf) -> bool {
        user_info(key).await.is_ok_and(|info| info.metered_plan())
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let info = user_info(&key).await?;
        let (snapshot, label) = info.metered_quota().ok_or(Error::NoQuota)?;
        Ok(CopilotMetered {
            plan: info.plan(),
            quota: label.to_owned(),
            used: snapshot.used(),
            limit: snapshot.entitlement,
            remaining: snapshot.quota_remaining,
            percent_used: 100.0 - snapshot.percent_remaining,
            reset: quota_reset(&info),
        })
    }
}

impl UserInfo {
    fn metered_plan(&self) -> bool {
        self.limited_user_quotas.is_none()
    }

    fn metered_quota(&self) -> Option<(&QuotaSnapshot, &'static str)> {
        let snapshots = self.quota_snapshots.as_ref()?;
        usable(snapshots.premium_interactions.as_ref(), "Premium requests")
            .or_else(|| usable(snapshots.chat.as_ref(), "Chat"))
    }
}

impl QuotaSnapshot {
    #[allow(clippy::cast_precision_loss)]
    fn used(&self) -> f64 {
        self.entitlement as f64 - self.quota_remaining
    }
}

fn usable<'a>(
    snapshot: Option<&'a QuotaSnapshot>,
    label: &'static str,
) -> Option<(&'a QuotaSnapshot, &'static str)> {
    snapshot
        .filter(|snap| snap.has_quota && snap.entitlement > 0)
        .map(|snap| (snap, label))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN_BILLED_USER_INFO: &str = r#"{
        "access_type_sku": "free_limited_copilot",
        "copilot_plan": "individual",
        "chat_enabled": true,
        "quota_snapshots": {
            "chat": {
                "overage_count": 0, "overage_permitted": false,
                "percent_remaining": 99.9, "quota_id": "chat",
                "quota_remaining": 199.8, "unlimited": false,
                "timestamp_utc": "2026-09-29T06:17:14.714-07:00",
                "has_quota": true, "quota_reset_at": 0,
                "token_based_billing": true, "credits_used": 0,
                "remaining": 199, "entitlement": 200
            },
            "completions": {
                "overage_count": 0, "overage_permitted": false,
                "percent_remaining": 100.0, "quota_id": "completions",
                "quota_remaining": 2000.0, "unlimited": false,
                "timestamp_utc": "2026-09-29T06:17:14.714-07:00",
                "has_quota": true, "quota_reset_at": 0,
                "token_based_billing": true, "credits_used": 0,
                "remaining": 2000, "entitlement": 2000
            },
            "premium_interactions": {
                "overage_count": 0, "overage_permitted": false,
                "percent_remaining": 0.0, "quota_id": "premium_interactions",
                "quota_remaining": 0.0, "unlimited": false,
                "timestamp_utc": "2026-09-29T06:17:14.714-07:00",
                "has_quota": false, "quota_reset_at": 0,
                "token_based_billing": true, "credits_used": 0,
                "remaining": 0, "entitlement": 0
            }
        },
        "token_based_billing": true,
        "quota_reset_date": "2026-10-01",
        "quota_reset_date_utc": "2026-10-01T00:00:00.000Z"
    }"#;

    #[test]
    fn token_billed_account_is_metered_on_its_chat_snapshot() {
        let info: UserInfo = serde_json::from_str(TOKEN_BILLED_USER_INFO).unwrap();
        assert!(info.metered_plan());
        assert_eq!(info.plan(), "free_limited_copilot");
        let (snapshot, label) = info.metered_quota().unwrap();
        assert_eq!(label, "Chat");
        assert_eq!(snapshot.entitlement, 200);
        assert!((snapshot.used() - 0.2).abs() < 1e-9);
        assert!((100.0 - snapshot.percent_remaining - 0.1).abs() < 1e-9);
        assert_eq!(
            quota_reset(&info),
            "2026-10-01T00:00:00Z".parse::<DateTime<Utc>>().ok()
        );
    }
}
