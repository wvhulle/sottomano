use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    future::{Future, ready},
    path::PathBuf,
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{
    Error, subscription_login,
    usage_endpoint::{self, PlanUsage},
};
use crate::data::source::Source;

const DEFAULT_DECIMAL_PLACES: i32 = 2;

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
    decimal_places: Option<i32>,
    disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WindowUsage {
    pub utilization: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Money {
    pub amount: f64,
    pub currency: String,
    pub decimal_places: usize,
}

impl Money {
    pub fn from_minor_units(minor_units: f64, currency: String, decimal_places: i32) -> Self {
        Money {
            amount: minor_units / 10f64.powi(decimal_places),
            currency,
            decimal_places: usize::try_from(decimal_places).unwrap_or_default(),
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
    Disabled(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtraUsage {
    pub status: ExtraStatus,
    pub spent: Option<Money>,
    pub monthly_limit: Option<Money>,
    pub used_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeUsageWindows {
    pub plan: String,
    pub session: WindowUsage,
    pub weekly: WindowUsage,
    pub sonnet: Option<WindowUsage>,
    pub extra: Option<ExtraUsage>,
}

impl ClaudeUsageWindows {
    pub fn session_used(&self) -> f64 {
        self.session.utilization
    }
}

impl Source for ClaudeUsageWindows {
    type Error = Error;
    type Key = PathBuf;

    const NAME: &'static str = "claude-usage-windows";
    const INTERVAL: Duration = Duration::from_mins(1);

    fn available(key: &PathBuf) -> impl Future<Output = bool> + Send {
        ready(subscription_login(key).is_some_and(|oauth| !oauth.is_enterprise()))
    }

    async fn poll(key: PathBuf) -> Result<Self, Self::Error> {
        let PlanUsage { plan, usage } = usage_endpoint::fetch::<Usage>(&key).await?;
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
            used_percent: self.utilization,
        })
    }

    fn status(&self) -> ExtraStatus {
        if self.is_enabled {
            return ExtraStatus::Enabled;
        }
        ExtraStatus::Disabled(
            self.disabled_reason
                .as_deref()
                .unwrap_or("disabled")
                .replace('_', " "),
        )
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
        assert!((usage.five_hour.utilization - 42.0).abs() < f64::EPSILON);
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
        assert!((window.utilization - 4.5).abs() < f64::EPSILON);
        assert!(window.resets_at.is_some());
    }

    #[test]
    fn exhausted_extra_credits_are_reported_in_their_currency() {
        let overage: Overage = serde_json::from_str(OUT_OF_CREDITS_EXTRA).unwrap();
        let extra = overage.extra_usage().unwrap();
        assert_eq!(
            extra.status,
            ExtraStatus::Disabled("out of credits".to_owned())
        );
        assert_eq!(extra.spent.unwrap().to_string(), "20.16 EUR");
        assert_eq!(extra.monthly_limit.unwrap().to_string(), "30.00 EUR");
        assert_eq!(extra.used_percent, Some(67.2));
    }
}
