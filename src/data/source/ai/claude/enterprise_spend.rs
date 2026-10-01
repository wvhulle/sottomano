use std::{
    future::{Future, ready},
    path::PathBuf,
    time::Duration,
};

use serde::Deserialize;

use super::{
    Error, ExtraStatus, Money, Oauth, subscription_login,
    usage_endpoint::{self, PlanUsage},
};
use crate::data::source::Source;

#[derive(Deserialize)]
struct Usage {
    spend: Spend,
}

#[derive(Deserialize)]
struct Spend {
    used: MinorAmount,
    limit: Option<MinorAmount>,
    #[serde(default)]
    percent: f64,
    #[serde(default)]
    enabled: bool,
    disabled_reason: Option<String>,
}

#[derive(Deserialize)]
struct MinorAmount {
    amount_minor: f64,
    currency: String,
    exponent: i32,
}

impl From<MinorAmount> for Money {
    fn from(amount: MinorAmount) -> Self {
        Money::from_minor_units(amount.amount_minor, amount.currency, amount.exponent)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeEnterpriseSpend {
    pub plan: String,
    pub status: ExtraStatus,
    pub spent: Money,
    pub limit: Option<Money>,
    pub used_percent: f64,
}

impl Source for ClaudeEnterpriseSpend {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-enterprise-spend";
    const INTERVAL: Duration = Duration::from_mins(5);

    fn available(key: &PathBuf) -> impl Future<Output = bool> + Send {
        ready(
            subscription_login(key)
                .as_ref()
                .is_some_and(Oauth::is_enterprise),
        )
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let PlanUsage { plan, usage } = usage_endpoint::fetch::<Usage>(&key).await?;
        Ok(usage.spend.into_enterprise(plan))
    }
}

impl Spend {
    fn into_enterprise(self, plan: String) -> ClaudeEnterpriseSpend {
        let status = if self.enabled {
            ExtraStatus::Enabled
        } else {
            ExtraStatus::Disabled(
                self.disabled_reason
                    .as_deref()
                    .unwrap_or("disabled")
                    .replace('_', " "),
            )
        };
        ClaudeEnterpriseSpend {
            plan,
            status,
            spent: self.used.into(),
            limit: self.limit.map(Money::from),
            used_percent: self.percent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTERPRISE_USAGE: &str = r#"{
        "five_hour": null, "seven_day": null, "seven_day_sonnet": null,
        "extra_usage": {
            "is_enabled": true, "monthly_limit": null, "used_credits": 179.0,
            "utilization": null, "currency": "USD", "decimal_places": 2
        },
        "limits": [],
        "spend": {
            "used": { "amount_minor": 179, "currency": "USD", "exponent": 2 },
            "limit": null, "percent": 0, "severity": "normal",
            "enabled": true, "disabled_reason": null
        }
    }"#;

    #[test]
    fn enterprise_usage_without_windows_reports_spend() {
        let usage: Usage = serde_json::from_str(ENTERPRISE_USAGE).unwrap();
        let enterprise = usage.spend.into_enterprise("Enterprise".to_owned());
        assert_eq!(enterprise.status, ExtraStatus::Enabled);
        assert_eq!(enterprise.spent.to_string(), "1.79 USD");
        assert_eq!(enterprise.spent.short_text(), "$1.79");
        assert!(enterprise.limit.is_none());
    }
}
