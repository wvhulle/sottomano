use std::borrow::Cow;

use chrono::{DateTime, Datelike, Days, Months, NaiveTime, TimeDelta, Utc};
use sottomano_config::severity::Severity;
use uom::si::{
    f64::Ratio,
    ratio::{percent, ratio},
};

const MIN_ELAPSED_PERCENT: f64 = 10.0;
const CRITICAL_USED_PERCENT: f64 = 90.0;
const FULL_PERCENT: f64 = 100.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Quota {
    pub label: Cow<'static, str>,
    pub used: Ratio,
    pub elapsed: Ratio,
    pub starts_at: DateTime<Utc>,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pace {
    Lasts { at_reset: Ratio },
    RunsOut { at: DateTime<Utc> },
}

impl Quota {
    pub fn worst_severity(quotas: &[Quota]) -> Option<Severity> {
        quotas.iter().map(Quota::severity).max()
    }

    pub fn within_window(
        label: impl Into<Cow<'static, str>>,
        used: Ratio,
        resets_at: DateTime<Utc>,
        length: TimeDelta,
        now: DateTime<Utc>,
    ) -> Self {
        let starts_at = resets_at - length;
        let elapsed = (now - starts_at).as_seconds_f64() / length.as_seconds_f64();
        Quota {
            label: label.into(),
            used,
            elapsed: Ratio::new::<ratio>(elapsed.clamp(0.0, 1.0)),
            starts_at,
            resets_at,
        }
    }

    pub fn within_month_before(
        label: impl Into<Cow<'static, str>>,
        used: Ratio,
        resets_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Option<Self> {
        let starts_at = resets_at.checked_sub_months(Months::new(1))?;
        Some(Self::within_window(
            label,
            used,
            resets_at,
            resets_at - starts_at,
            now,
        ))
    }

    pub fn within_calendar_month(
        label: impl Into<Cow<'static, str>>,
        used: Ratio,
        now: DateTime<Utc>,
    ) -> Option<Self> {
        let first_of_month = now.date_naive() - Days::new(u64::from(now.day0()));
        let resets_at = first_of_month
            .checked_add_months(Months::new(1))?
            .and_time(NaiveTime::MIN)
            .and_utc();
        Self::within_month_before(label, used, resets_at, now)
    }

    #[must_use]
    pub fn projected_at_reset(&self) -> Option<Ratio> {
        let measurable = self.used.get::<percent>() > 0.0
            && self.elapsed >= Ratio::new::<percent>(MIN_ELAPSED_PERCENT);
        measurable.then(|| self.used / self.elapsed)
    }

    #[must_use]
    pub fn pace(&self) -> Option<Pace> {
        let at_reset = self.projected_at_reset()?;
        if at_reset <= Ratio::new::<percent>(FULL_PERCENT) {
            return Some(Pace::Lasts { at_reset });
        }
        let full_share = (self.elapsed / self.used).get::<ratio>();
        let until_full = (self.resets_at - self.starts_at)
            .to_std()
            .ok()?
            .mul_f64(full_share);
        Some(Pace::RunsOut {
            at: self.starts_at + TimeDelta::from_std(until_full).ok()?,
        })
    }

    #[must_use]
    pub fn severity(&self) -> Severity {
        match self.pace() {
            None | Some(Pace::Lasts { .. }) => Severity::Normal,
            Some(Pace::RunsOut { .. })
                if self.used > Ratio::new::<percent>(CRITICAL_USED_PERCENT) =>
            {
                Severity::Critical
            }
            Some(Pace::RunsOut { .. }) => Severity::Warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_LENGTH: TimeDelta = TimeDelta::hours(5);

    fn session(used_percent: f64, elapsed: TimeDelta) -> Quota {
        let start: DateTime<Utc> = "2026-10-02T10:00:00Z".parse().unwrap();
        Quota::within_window(
            "Session",
            Ratio::new::<percent>(used_percent),
            start + SESSION_LENGTH,
            SESSION_LENGTH,
            start + elapsed,
        )
    }

    #[test]
    fn too_early_in_the_window_projects_nothing() {
        let quota = session(8.0, TimeDelta::minutes(20));
        assert_eq!(quota.projected_at_reset(), None);
        assert_eq!(quota.severity(), Severity::Normal);
    }

    #[test]
    fn a_steady_pace_lasts_until_reset() {
        let quota = session(40.0, TimeDelta::minutes(150));
        let projected = quota.projected_at_reset().unwrap().get::<percent>();
        assert!((projected - 80.0).abs() < 1e-9);
        assert_eq!(quota.severity(), Severity::Normal);
    }

    #[test]
    fn a_fast_pace_warns_before_running_out() {
        let quota = session(60.0, TimeDelta::hours(2));
        assert_eq!(
            quota.pace(),
            Some(Pace::RunsOut {
                at: quota.starts_at + TimeDelta::minutes(200)
            })
        );
        assert_eq!(quota.severity(), Severity::Warning);
    }

    #[test]
    fn a_fast_pace_near_the_limit_is_critical() {
        assert_eq!(
            session(92.0, TimeDelta::hours(3)).severity(),
            Severity::Critical
        );
    }

    #[test]
    fn calendar_months_reset_on_the_first() {
        let now: DateTime<Utc> = "2026-12-16T12:00:00Z".parse().unwrap();
        let quota =
            Quota::within_calendar_month("Extra", Ratio::new::<percent>(50.0), now).unwrap();
        assert_eq!(
            quota.resets_at,
            "2027-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
    }
}
