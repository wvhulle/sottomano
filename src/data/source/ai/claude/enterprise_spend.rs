use std::{
    future::{Future, ready},
    path::PathBuf,
    time::Duration,
};

use serde::Deserialize;

use super::{
    Error, ExtraStatus, Money, Oauth,
    oauth_api::{self, PlanReply},
    subscription_login, title_case,
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
    percent: f64,
    enabled: bool,
    disabled_reason: Option<String>,
}

#[derive(Deserialize)]
struct MinorAmount {
    amount_minor: f64,
    currency: String,
    exponent: u8,
}

#[derive(Deserialize)]
struct Profile {
    organization: Organization,
}

#[derive(Deserialize)]
struct Organization {
    name: String,
    seat_tier: String,
    subscription_status: String,
}

impl From<MinorAmount> for Money {
    fn from(amount: MinorAmount) -> Self {
        Money::from_minor_units(amount.amount_minor, amount.currency, amount.exponent)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeEnterpriseSpend {
    pub organization: String,
    pub seat_tier: String,
    pub subscription_status: String,
    pub status: ExtraStatus,
    pub used: Money,
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
        let PlanReply { reply: usage, .. } =
            oauth_api::fetch::<Usage>(&key, oauth_api::USAGE_URL).await?;
        let PlanReply { reply: profile, .. } =
            oauth_api::fetch::<Profile>(&key, oauth_api::PROFILE_URL).await?;
        Ok(ClaudeEnterpriseSpend::of(usage.spend, profile.organization))
    }
}

impl ClaudeEnterpriseSpend {
    fn of(spend: Spend, organization: Organization) -> Self {
        let status = if spend.enabled {
            ExtraStatus::Enabled
        } else {
            ExtraStatus::Disabled {
                reason: spend
                    .disabled_reason
                    .as_deref()
                    .map(|reason| reason.replace('_', " ")),
            }
        };
        ClaudeEnterpriseSpend {
            organization: organization.name,
            seat_tier: title_case(&organization.seat_tier.replace('_', " ")),
            subscription_status: organization.subscription_status.replace('_', " "),
            status,
            used: spend.used.into(),
            limit: spend.limit.map(Money::from),
            used_percent: spend.percent,
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

    const ENTERPRISE_PROFILE: &str = r#"{
        "account": { "display_name": "Someone", "has_claude_max": false },
        "organization": {
            "name": "Example Corp", "organization_type": "claude_enterprise",
            "seat_tier": "enterprise_usage_based", "subscription_status": "active",
            "has_extra_usage_enabled": true
        }
    }"#;

    fn enterprise_spend() -> ClaudeEnterpriseSpend {
        let usage: Usage = serde_json::from_str(ENTERPRISE_USAGE).unwrap();
        let profile: Profile = serde_json::from_str(ENTERPRISE_PROFILE).unwrap();
        ClaudeEnterpriseSpend::of(usage.spend, profile.organization)
    }

    #[test]
    fn enterprise_usage_without_windows_reports_spend() {
        let enterprise = enterprise_spend();
        assert_eq!(enterprise.status, ExtraStatus::Enabled);
        assert_eq!(enterprise.used.to_string(), "1.79 USD");
        assert_eq!(enterprise.used.short_text(), "$1.79");
        assert!(enterprise.limit.is_none());
    }

    #[test]
    fn profile_names_the_organization_and_seat_tier() {
        let enterprise = enterprise_spend();
        assert_eq!(enterprise.organization, "Example Corp");
        assert_eq!(enterprise.seat_tier, "Enterprise Usage Based");
        assert_eq!(enterprise.subscription_status, "active");
    }

    #[test]
    fn a_spend_without_its_percent_is_rejected() {
        let missing_percent = r#"{"spend": {
            "used": { "amount_minor": 1, "currency": "USD", "exponent": 2 },
            "limit": null, "enabled": true, "disabled_reason": null
        }}"#;
        assert!(serde_json::from_str::<Usage>(missing_percent).is_err());
    }
}
