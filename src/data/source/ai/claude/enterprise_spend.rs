use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Utc};
use iced::futures::Stream;
use rust_decimal::Decimal;
use rusty_money::iso;
use serde::Deserialize;
use uom::si::{f64::Ratio, ratio::percent};

use super::{
    Error, ExtraStatus, LoginKind, login_polls,
    oauth_api::{self, PlanReply},
    title_case,
};
use crate::data::{
    money::{Money, from_minor_units},
    source::{DeliveryCadence, ReadingSource, SourceDelivery, ai::quota::Quota},
};

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
    amount_minor: Decimal,
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

impl TryFrom<MinorAmount> for Money {
    type Error = Error;

    fn try_from(amount: MinorAmount) -> Result<Self, Self::Error> {
        let Some(currency) = iso::find(&amount.currency) else {
            return Err(Error::UnknownCurrency(amount.currency));
        };
        Ok(from_minor_units(
            amount.amount_minor,
            currency,
            u32::from(amount.exponent),
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeEnterpriseSpend {
    pub status: ExtraStatus,
    pub used: Money,
    pub limit: Option<Money>,
    pub used_share: Ratio,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeOrganization {
    pub organization: String,
    pub seat_tier: String,
    pub subscription_status: String,
}

impl From<Organization> for ClaudeOrganization {
    fn from(organization: Organization) -> Self {
        ClaudeOrganization {
            organization: organization.name,
            seat_tier: title_case(&organization.seat_tier.replace('_', " ")),
            subscription_status: organization.subscription_status.replace('_', " "),
        }
    }
}

impl ReadingSource for ClaudeOrganization {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-organization";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_mins(5));

    fn delivery_stream(key: PathBuf) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        login_polls(key, LoginKind::Enterprise, Self::poll)
    }
}

impl ClaudeOrganization {
    async fn poll(key: PathBuf) -> Result<Self, Error> {
        let PlanReply { reply: profile, .. } =
            oauth_api::fetch::<Profile>(&key, oauth_api::PROFILE_URL).await?;
        Ok(profile.organization.into())
    }
}

impl ReadingSource for ClaudeEnterpriseSpend {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-enterprise-spend";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_mins(5));

    fn delivery_stream(key: PathBuf) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        login_polls(key, LoginKind::Enterprise, Self::poll)
    }
}

impl ClaudeEnterpriseSpend {
    async fn poll(key: PathBuf) -> Result<Self, Error> {
        let PlanReply { reply: usage, .. } =
            oauth_api::fetch::<Usage>(&key, oauth_api::USAGE_URL).await?;
        ClaudeEnterpriseSpend::try_from(usage.spend)
    }
}

impl ClaudeEnterpriseSpend {
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.limit
            .as_ref()
            .and_then(|_| {
                Quota::within_calendar_month("Claude monthly spend limit", self.used_share, now)
            })
            .into_iter()
            .collect()
    }
}

impl TryFrom<Spend> for ClaudeEnterpriseSpend {
    type Error = Error;

    fn try_from(spend: Spend) -> Result<Self, Self::Error> {
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
        Ok(ClaudeEnterpriseSpend {
            status,
            used: Money::try_from(spend.used)?,
            limit: spend.limit.map(Money::try_from).transpose()?,
            used_share: Ratio::new::<percent>(spend.percent),
        })
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
        ClaudeEnterpriseSpend::try_from(usage.spend).unwrap()
    }

    #[test]
    fn enterprise_usage_without_windows_reports_spend() {
        let enterprise = enterprise_spend();
        assert_eq!(enterprise.status, ExtraStatus::Enabled);
        assert_eq!(enterprise.used.to_string(), "$1.79");
        assert!(enterprise.limit.is_none());
    }

    #[test]
    fn profile_names_the_organization_and_seat_tier() {
        let profile: Profile = serde_json::from_str(ENTERPRISE_PROFILE).unwrap();
        let enterprise = ClaudeOrganization::from(profile.organization);
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
