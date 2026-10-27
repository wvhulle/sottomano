use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, Local, Utc};
use futures::Stream;
use rust_decimal::Decimal;
use rusty_money::iso;
use serde::Deserialize;
use uom::si::{f64::Ratio, ratio::percent};

use super::{
    Error, ExtraStatus, LoginKind, login_polls,
    oauth_usage_api::{self, PlanReply},
    title_case,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        spend_money::{Money, from_minor_units},
        usage_quota::Quota,
        working_time_forecast::{CalendarSpan, SpendPacing, WorkingTime},
    },
    delivery_cadence_drivers::PeriodicPolling,
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
    pub working_time: WorkingTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaudeEnterpriseAccount {
    pub credentials: PathBuf,
    pub working_time: WorkingTime,
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

impl SnapshotSource for ClaudeOrganization {
    type Error = Error;
    type StreamInputs = PathBuf;

    const NAME: &'static str = "claude-organization";

    fn delivery_stream(inputs: PathBuf) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        login_polls(
            PeriodicPolling::every(Duration::from_mins(5)),
            inputs,
            LoginKind::Enterprise,
            Self::poll,
        )
    }
}

impl ClaudeOrganization {
    async fn poll(key: PathBuf) -> Result<Self, Error> {
        let PlanReply { reply: profile, .. } =
            oauth_usage_api::fetch::<Profile>(&key, oauth_usage_api::PROFILE_URL).await?;
        Ok(profile.organization.into())
    }
}

impl SnapshotSource for ClaudeEnterpriseSpend {
    type Error = Error;
    type StreamInputs = ClaudeEnterpriseAccount;

    const NAME: &'static str = "claude-enterprise-spend";

    fn delivery_stream(
        account: ClaudeEnterpriseAccount,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let working_time = account.working_time;
        login_polls(
            PeriodicPolling::every(Duration::from_mins(5)),
            account.credentials,
            LoginKind::Enterprise,
            move |key| Self::poll(key, working_time.clone()),
        )
    }
}

impl ClaudeEnterpriseSpend {
    async fn poll(key: PathBuf, working_time: WorkingTime) -> Result<Self, Error> {
        let PlanReply { reply: usage, .. } =
            oauth_usage_api::fetch::<Usage>(&key, oauth_usage_api::USAGE_URL).await?;
        ClaudeEnterpriseSpend::paced_by(usage.spend, working_time)
    }

    #[must_use]
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        let now = now.with_timezone(&Local);
        self.limit
            .as_ref()
            .map(|_| {
                CalendarSpan::month_containing(now.date_naive()).paced_quota(
                    "Claude monthly spend limit",
                    self.used_share,
                    &SpendPacing::WorkingTime(self.working_time.clone()),
                    now,
                )
            })
            .into_iter()
            .collect()
    }

    fn paced_by(spend: Spend, working_time: WorkingTime) -> Result<Self, Error> {
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
            working_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::WeekdaySet;
    use sottomano_config::calendar::WorkingHours;

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
        let working_time = WorkingTime {
            days: WeekdaySet::ALL,
            hours: WorkingHours::default(),
        };
        ClaudeEnterpriseSpend::paced_by(usage.spend, working_time).unwrap()
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
