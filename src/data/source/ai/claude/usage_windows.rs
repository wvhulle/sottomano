use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    future::{Future, ready},
    path::PathBuf,
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;

use super::{
    Error,
    oauth_api::{self, PlanReply},
    subscription_login,
};
use crate::data::source::Source;

const DEFAULT_DECIMAL_PLACES: u8 = 2;
const FULL_PERCENT: f64 = 100.0;
const SESSION_LENGTH: TimeDelta = TimeDelta::hours(5);
const WEEKLY_LENGTH: TimeDelta = TimeDelta::days(7);
const MIN_ELAPSED: TimeDelta = TimeDelta::minutes(10);

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
    pub utilization: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pace {
    Lasts { at_reset: f64 },
    RunsOut { at: DateTime<Utc> },
}

impl WindowUsage {
    pub fn pace(&self, length: TimeDelta, now: DateTime<Utc>) -> Option<Pace> {
        let start = self.resets_at? - length;
        let elapsed = now - start;
        if self.utilization <= 0.0 || elapsed < MIN_ELAPSED {
            return None;
        }
        let at_reset = self.utilization * length.as_seconds_f64() / elapsed.as_seconds_f64();
        if at_reset <= FULL_PERCENT {
            return Some(Pace::Lasts { at_reset });
        }
        let until_full = elapsed
            .to_std()
            .ok()?
            .mul_f64(FULL_PERCENT / self.utilization);
        Some(Pace::RunsOut {
            at: start + TimeDelta::from_std(until_full).ok()?,
        })
    }
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
    pub used_percent: Option<f64>,
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
    pub fn session_used(&self) -> f64 {
        self.session.utilization
    }

    pub fn session_pace(&self, now: DateTime<Utc>) -> Option<Pace> {
        self.session.pace(SESSION_LENGTH, now)
    }

    pub fn weekly_pace(&self, now: DateTime<Utc>) -> Option<Pace> {
        self.weekly.pace(WEEKLY_LENGTH, now)
    }

    pub fn sonnet_pace(&self, now: DateTime<Utc>) -> Option<Pace> {
        self.sonnet.as_ref()?.pace(WEEKLY_LENGTH, now)
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
            used_percent: self.utilization,
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

    fn session_at(utilization: f64, start: DateTime<Utc>) -> WindowUsage {
        WindowUsage {
            utilization,
            resets_at: Some(start + SESSION_LENGTH),
        }
    }

    fn session_start() -> DateTime<Utc> {
        "2026-10-02T10:00:00Z".parse().unwrap()
    }

    #[test]
    fn a_steady_pace_projects_usage_at_reset() {
        let start = session_start();
        let pace = session_at(40.0, start).pace(SESSION_LENGTH, start + TimeDelta::minutes(150));
        let Some(Pace::Lasts { at_reset }) = pace else {
            panic!("expected the session to last, got {pace:?}");
        };
        assert!((at_reset - 80.0).abs() < 1e-9);
    }

    #[test]
    fn a_fast_pace_runs_out_before_reset() {
        let start = session_start();
        let pace = session_at(60.0, start).pace(SESSION_LENGTH, start + TimeDelta::hours(2));
        assert_eq!(
            pace,
            Some(Pace::RunsOut {
                at: start + TimeDelta::minutes(200)
            })
        );
    }

    #[test]
    fn too_early_in_the_window_projects_nothing() {
        let start = session_start();
        let pace = session_at(5.0, start).pace(SESSION_LENGTH, start + TimeDelta::minutes(5));
        assert_eq!(pace, None);
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
        assert_eq!(extra.used_percent, Some(67.2));
    }
}
