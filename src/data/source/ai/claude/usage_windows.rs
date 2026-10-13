use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};
use iced::futures::Stream;
use serde::{Deserialize, Deserializer};
use uom::si::{f64::Ratio, ratio::percent};

use super::{
    Error, LoginKind, login_polls,
    oauth_api::{self, PlanReply},
};
use crate::data::source::{
    Delivery, PolledSource, Source,
    ai::quota::{Quota, QuotaLimited},
};

const DEFAULT_DECIMAL_PLACES: u8 = 2;
const SESSION_LENGTH: TimeDelta = TimeDelta::hours(5);
const WEEKLY_LENGTH: TimeDelta = TimeDelta::days(7);

#[derive(Deserialize)]
struct Usage {
    five_hour: WindowUsage,
    seven_day: WindowUsage,
    seven_day_sonnet: Option<WindowUsage>,
    #[serde(rename = "extra_usage")]
    overage: Option<Overage>,
}

#[derive(Deserialize)]
struct Overage {
    #[serde(default)]
    is_enabled: bool,
    used_credits: Option<f64>,
    monthly_limit: Option<f64>,
    utilization: Option<f64>,
    #[serde(default)]
    currency: String,
    decimal_places: Option<u8>,
    disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WindowUsage {
    #[serde(deserialize_with = "percent_share")]
    pub utilization: Ratio,
    pub resets_at: Option<DateTime<Utc>>,
}

fn percent_share<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Ratio, D::Error> {
    f64::deserialize(deserializer).map(Ratio::new::<percent>)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Money {
    pub amount: f64,
    pub currency: String,
    pub decimal_places: usize,
}

impl Money {
    pub fn from_minor_units(minor_units: f64, currency: String, decimal_places: u8) -> Self {
        Money {
            amount: minor_units / 10f64.powi(i32::from(decimal_places)),
            currency,
            decimal_places: usize::from(decimal_places),
        }
    }

    pub fn short_text(&self) -> String {
        match self.currency.as_str() {
            "USD" => format!("${:.*}", self.decimal_places, self.amount),
            _ => self.to_string(),
        }
    }
}

impl Display for Money {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "{:.*} {}",
            self.decimal_places, self.amount, self.currency
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtraStatus {
    Enabled,
    Disabled { reason: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtraUsage {
    pub status: ExtraStatus,
    pub spent: Option<Money>,
    pub monthly_limit: Option<Money>,
    pub used_share: Option<Ratio>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeUsageWindows {
    pub plan: Option<String>,
    pub session: WindowUsage,
    pub weekly: WindowUsage,
    pub sonnet: Option<WindowUsage>,
    pub extra: Option<ExtraUsage>,
}

impl ClaudeUsageWindows {
    pub fn session_used(&self) -> Ratio {
        self.session.utilization
    }

    pub fn session_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        self.session.quota("Claude session", SESSION_LENGTH, now)
    }

    pub fn weekly_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        self.weekly.quota("Claude weekly limit", WEEKLY_LENGTH, now)
    }

    pub fn sonnet_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        self.sonnet
            .as_ref()?
            .quota("Claude weekly Sonnet limit", WEEKLY_LENGTH, now)
    }
}

impl WindowUsage {
    fn quota(&self, label: &'static str, length: TimeDelta, now: DateTime<Utc>) -> Option<Quota> {
        Some(Quota::within_window(
            label,
            self.utilization,
            self.resets_at?,
            length,
            now,
        ))
    }
}

impl QuotaLimited for ClaudeUsageWindows {
    fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        let extra = self
            .extra
            .as_ref()
            .filter(|extra| extra.status == ExtraStatus::Enabled && extra.monthly_limit.is_some())
            .and_then(|extra| extra.used_share)
            .and_then(|used| Quota::within_calendar_month("Claude extra usage", used, now));
        [
            self.session_quota(now),
            self.weekly_quota(now),
            self.sonnet_quota(now),
            extra,
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

impl Source for ClaudeUsageWindows {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-usage-windows";
}

impl PolledSource for ClaudeUsageWindows {
    const INTERVAL: Duration = Duration::from_mins(1);

    fn polls(key: PathBuf) -> impl Stream<Item = Delivery<Self>> + Send {
        login_polls::<Self>(key, LoginKind::Subscription)
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let PlanReply { plan, reply: usage } =
            oauth_api::fetch::<Usage>(&key, oauth_api::USAGE_URL).await?;
        Ok(ClaudeUsageWindows {
            plan,
            extra: usage.overage.as_ref().and_then(Overage::extra_usage),
            session: usage.five_hour,
            weekly: usage.seven_day,
            sonnet: usage.seven_day_sonnet,
        })
    }
}

impl Overage {
    fn extra_usage(&self) -> Option<ExtraUsage> {
        let ever_used = self.used_credits.is_some_and(|credits| credits > 0.0);
        (self.is_enabled || ever_used).then(|| ExtraUsage {
            status: self.status(),
            spent: self.used_credits.map(|minor| self.money(minor)),
            monthly_limit: self.monthly_limit.map(|minor| self.money(minor)),
            used_share: self.utilization.map(Ratio::new::<percent>),
        })
    }

    fn status(&self) -> ExtraStatus {
        if self.is_enabled {
            return ExtraStatus::Enabled;
        }
        ExtraStatus::Disabled {
            reason: self
                .disabled_reason
                .as_deref()
                .map(|reason| reason.replace('_', " ")),
        }
    }

    fn money(&self, minor_units: f64) -> Money {
        Money::from_minor_units(
            minor_units,
            self.currency.clone(),
            self.decimal_places.unwrap_or(DEFAULT_DECIMAL_PLACES),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = r#"{
        "five_hour": {
            "utilization": 42.0, "resets_at": "2026-08-27T12:00:00Z",
            "limit_dollars": null, "used_dollars": null
        },
        "seven_day": { "utilization": 49.0, "resets_at": "2026-09-01T00:00:00Z" },
        "seven_day_sonnet": null,
        "extra_usage": {
            "is_enabled": false, "monthly_limit": null,
            "used_credits": null, "utilization": null
        },
        "limits": {}
    }"#;

    const OUT_OF_CREDITS_EXTRA: &str = r#"{
        "is_enabled": false, "monthly_limit": 3000, "used_credits": 2016.0,
        "utilization": 67.2, "currency": "EUR", "decimal_places": 2,
        "disabled_reason": "out_of_credits", "user_disabled": false
    }"#;

    #[test]
    fn usage_windows_decode_with_null_sonnet_and_disabled_overage() {
        let usage: Usage = serde_json::from_str(USAGE).unwrap();
        assert!((usage.five_hour.utilization.get::<percent>() - 42.0).abs() < 1e-9);
        assert_eq!(
            usage.seven_day.resets_at,
            "2026-09-01T00:00:00Z".parse::<DateTime<Utc>>().ok()
        );
        assert!(usage.seven_day_sonnet.is_none());
        assert!(
            usage
                .overage
                .as_ref()
                .and_then(Overage::extra_usage)
                .is_none()
        );
    }

    #[test]
    fn microsecond_offset_reset_times_decode() {
        let window: WindowUsage = serde_json::from_str(
            r#"{"utilization": 4.5, "resets_at": "2026-09-29T18:20:00.156123+00:00"}"#,
        )
        .unwrap();
        assert!((window.utilization.get::<percent>() - 4.5).abs() < 1e-9);
        assert!(window.resets_at.is_some());
    }

    #[test]
    fn exhausted_extra_credits_are_reported_in_their_currency() {
        let overage: Overage = serde_json::from_str(OUT_OF_CREDITS_EXTRA).unwrap();
        let extra = overage.extra_usage().unwrap();
        assert_eq!(
            extra.status,
            ExtraStatus::Disabled {
                reason: Some("out of credits".to_owned())
            }
        );
        assert_eq!(extra.spent.unwrap().to_string(), "20.16 EUR");
        assert_eq!(extra.monthly_limit.unwrap().to_string(), "30.00 EUR");
        let used_percent = extra.used_share.map(|used| used.get::<percent>());
        assert!(used_percent.is_some_and(|used| (used - 67.2).abs() < 1e-9));
    }
}
