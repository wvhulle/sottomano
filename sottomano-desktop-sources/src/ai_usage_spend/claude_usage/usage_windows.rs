use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use futures::Stream;
use rust_decimal::Decimal;
use rusty_money::iso;
use serde::Deserialize;
use uom::si::{f64::Ratio, ratio::percent};

use super::{
    Error, LoginKind, login_polls,
    oauth_usage_api::{self, PlanReply},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    ai_usage_spend::{
        spend_money::{Money, minor_units_money},
        subscription_plan::SubscriptionPlan,
        usage_quota::Quota,
    },
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{
        dbus_desktop_buses::OnBus, http_json_client::deserialize_percent_ratio,
    },
};

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
    used_credits: Option<Decimal>,
    monthly_limit: Option<Decimal>,
    utilization: Option<f64>,
    #[serde(default)]
    currency: String,
    decimal_places: Option<u8>,
    disabled_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WindowUsage {
    #[serde(deserialize_with = "deserialize_percent_ratio")]
    pub utilization: Ratio,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtraStatus {
    Enabled,
    Disabled,
    DisabledBecause(String),
}

impl ExtraStatus {
    pub(crate) fn disabled(disabled_reason: Option<&str>) -> Self {
        disabled_reason.map_or(ExtraStatus::Disabled, |reason| {
            ExtraStatus::DisabledBecause(reason.replace('_', " "))
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtraSpend {
    Spent(Money),
    Unreported,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtraAllowance {
    Capped {
        monthly_limit: Money,
        used_share: Ratio,
    },
    NoCapReported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtraUsage {
    pub status: ExtraStatus,
    pub spent: ExtraSpend,
    pub allowance: ExtraAllowance,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SonnetWindow {
    Tracked(WindowUsage),
    Untracked,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtraUsageOffer {
    Taken(ExtraUsage),
    NeverUsed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeUsageWindows {
    pub plan: SubscriptionPlan,
    pub session: WindowUsage,
    pub weekly: WindowUsage,
    pub sonnet: SonnetWindow,
    pub extra: ExtraUsageOffer,
}

impl ClaudeUsageWindows {
    #[must_use]
    pub fn fullest_window_used(&self) -> Ratio {
        self.all_usage_windows()
            .map(|window| window.utilization)
            .fold(Ratio::new::<percent>(0.0), Ratio::max)
    }

    #[must_use]
    pub fn session_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        self.session.quota("Claude session", SESSION_LENGTH, now)
    }

    #[must_use]
    pub fn weekly_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        self.weekly.quota("Claude weekly limit", WEEKLY_LENGTH, now)
    }

    fn all_usage_windows(&self) -> impl Iterator<Item = &WindowUsage> {
        let sonnet = match &self.sonnet {
            SonnetWindow::Tracked(sonnet) => Some(sonnet),
            SonnetWindow::Untracked => None,
        };
        [&self.session, &self.weekly].into_iter().chain(sonnet)
    }

    #[must_use]
    pub fn sonnet_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        let SonnetWindow::Tracked(sonnet) = &self.sonnet else {
            return None;
        };
        sonnet.quota("Claude weekly Sonnet limit", WEEKLY_LENGTH, now)
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

impl ClaudeUsageWindows {
    #[must_use]
    pub fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        let extra = self.extra_quota(now);
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

impl ClaudeUsageWindows {
    fn extra_quota(&self, now: DateTime<Utc>) -> Option<Quota> {
        let ExtraUsageOffer::Taken(ExtraUsage {
            status: ExtraStatus::Enabled,
            allowance: ExtraAllowance::Capped { used_share, .. },
            ..
        }) = &self.extra
        else {
            return None;
        };
        let window_used_up = self
            .all_usage_windows()
            .any(|window| window.utilization >= Ratio::new::<percent>(100.0));
        if !window_used_up {
            return None;
        }
        Quota::within_calendar_month("Claude extra usage", *used_share, now)
    }
}

impl SnapshotSource for ClaudeUsageWindows {
    type Error = Error;
    type StreamInputs = OnBus<PathBuf>;

    const NAME: &'static str = "Subscription windows";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        login_polls(
            PeriodicPolling::every(Duration::from_mins(1)),
            inputs,
            LoginKind::Subscription,
            Self::poll,
        )
    }
}

impl ClaudeUsageWindows {
    async fn poll(key: PathBuf) -> Result<Self, Error> {
        let PlanReply { plan, reply: usage } =
            oauth_usage_api::fetch::<Usage>(&key, oauth_usage_api::USAGE_URL).await?;
        Ok(ClaudeUsageWindows {
            plan,
            extra: usage
                .overage
                .as_ref()
                .map_or(ExtraUsageOffer::NeverUsed, Overage::extra_usage),
            session: usage.five_hour,
            weekly: usage.seven_day,
            sonnet: usage
                .seven_day_sonnet
                .map_or(SonnetWindow::Untracked, SonnetWindow::Tracked),
        })
    }
}

impl Overage {
    fn extra_usage(&self) -> ExtraUsageOffer {
        let ever_used = self
            .used_credits
            .is_some_and(|credits| credits > Decimal::ZERO);
        if !self.is_enabled && !ever_used {
            return ExtraUsageOffer::NeverUsed;
        }
        ExtraUsageOffer::Taken(ExtraUsage {
            status: if self.is_enabled {
                ExtraStatus::Enabled
            } else {
                ExtraStatus::disabled(self.disabled_reason.as_deref())
            },
            spent: self
                .used_credits
                .and_then(|minor| self.money(minor))
                .map_or(ExtraSpend::Unreported, ExtraSpend::Spent),
            allowance: self.allowance(),
        })
    }

    fn allowance(&self) -> ExtraAllowance {
        let Some(monthly_limit) = self.monthly_limit.and_then(|minor| self.money(minor)) else {
            return ExtraAllowance::NoCapReported;
        };
        let Some(utilization) = self.utilization else {
            return ExtraAllowance::NoCapReported;
        };
        ExtraAllowance::Capped {
            monthly_limit,
            used_share: Ratio::new::<percent>(utilization),
        }
    }

    fn money(&self, minor_units: Decimal) -> Option<Money> {
        let currency = iso::find(&self.currency)?;
        let exponent = self.decimal_places.map_or(currency.exponent, u32::from);
        Some(minor_units_money(minor_units, currency, exponent))
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
        assert_eq!(
            usage.overage.as_ref().map(Overage::extra_usage),
            Some(ExtraUsageOffer::NeverUsed)
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
        let ExtraUsageOffer::Taken(extra) = overage.extra_usage() else {
            panic!("exhausted credits count as taken extra usage");
        };
        assert_eq!(
            extra.status,
            ExtraStatus::DisabledBecause("out of credits".to_owned())
        );
        let ExtraSpend::Spent(spent) = extra.spent else {
            panic!("spent credits are reported");
        };
        assert_eq!(*spent.amount(), Decimal::new(2016, 2));
        let ExtraAllowance::Capped {
            monthly_limit,
            used_share,
        } = extra.allowance
        else {
            panic!("a monthly limit is reported");
        };
        assert_eq!(monthly_limit.currency().iso_alpha_code, "EUR");
        assert!((used_share.get::<percent>() - 67.2).abs() < 1e-9);
    }

    fn windows_with_weekly_use(weekly_percent: f64) -> ClaudeUsageWindows {
        let window = |used_percent: f64, resets_at: &str| WindowUsage {
            utilization: Ratio::new::<percent>(used_percent),
            resets_at: resets_at.parse().ok(),
        };
        ClaudeUsageWindows {
            plan: SubscriptionPlan::Unnamed,
            session: window(40.0, "2026-10-24T12:00:00Z"),
            weekly: window(weekly_percent, "2026-10-30T02:00:00Z"),
            sonnet: SonnetWindow::Untracked,
            extra: ExtraUsageOffer::Taken(ExtraUsage {
                status: ExtraStatus::Enabled,
                spent: ExtraSpend::Unreported,
                allowance: ExtraAllowance::Capped {
                    monthly_limit: Money::from_decimal(Decimal::from(30), iso::EUR),
                    used_share: Ratio::new::<percent>(98.0),
                },
            }),
        }
    }

    fn quota_labels(windows: &ClaudeUsageWindows) -> Vec<String> {
        let now: DateTime<Utc> = "2026-10-24T08:30:00Z".parse().unwrap();
        windows
            .quotas(now)
            .into_iter()
            .map(|quota| quota.label.to_string())
            .collect()
    }

    #[test]
    fn extra_usage_only_counts_once_a_subscription_window_is_used_up() {
        let extra_label = "Claude extra usage".to_owned();
        assert!(!quota_labels(&windows_with_weekly_use(27.0)).contains(&extra_label));
        assert!(quota_labels(&windows_with_weekly_use(100.0)).contains(&extra_label));
    }

    #[test]
    fn idle_session_still_reports_weekly_use() {
        let mut windows = windows_with_weekly_use(15.0);
        windows.session.utilization = Ratio::new::<percent>(0.0);
        assert!((windows.fullest_window_used().get::<percent>() - 15.0).abs() < 1e-9);
    }
}
