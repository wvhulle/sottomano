use std::borrow::Cow;

use chrono::{
    DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc, WeekdaySet,
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use sottomano_toml_config::{Config, calendar::WorkingHours};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::ai_usage_spend::{spend_money::Money, usage_quota::Quota};

const MIN_ELAPSED_DAYS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpendPacing {
    CalendarTime,
    WorkingTime(WorkingTime),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkingTime {
    pub days: WeekdaySet,
    pub hours: WorkingHours,
}

impl WorkingTime {
    #[must_use]
    pub fn new(config: &Config, days: WeekdaySet) -> Self {
        Self {
            days,
            hours: config.calendar.working_hours.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpendBudget {
    pub pacing: SpendPacing,
    pub monthly_limit_usd: u32,
}

#[derive(Debug, Clone, Copy)]
struct PeriodProgress {
    elapsed_days: f64,
    whole_days: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarSpan {
    pub since: NaiveDate,
    pub until: NaiveDate,
}

impl CalendarSpan {
    #[must_use]
    pub fn day(day: NaiveDate) -> Self {
        Self {
            since: day,
            until: day + TimeDelta::days(1),
        }
    }

    #[must_use]
    pub fn week_containing(day: NaiveDate) -> Self {
        let since = day - TimeDelta::days(i64::from(day.weekday().num_days_from_monday()));
        Self {
            since,
            until: since + TimeDelta::weeks(1),
        }
    }

    #[must_use]
    pub fn month_containing(day: NaiveDate) -> Self {
        let since = month_start(day);
        Self {
            since,
            until: month_start(since + TimeDelta::days(31)),
        }
    }

    #[must_use]
    pub fn contains(self, day: NaiveDate) -> bool {
        (self.since..self.until).contains(&day)
    }

    #[must_use]
    pub fn starts_at(self) -> DateTime<Utc> {
        local_midnight(self.since)
    }

    fn calendar_progress(self, now: DateTime<Local>) -> PeriodProgress {
        let starts_at = local_midnight(self.since);
        let whole = days_between(starts_at, local_midnight(self.until));
        PeriodProgress {
            elapsed_days: days_between(starts_at, now.with_timezone(&Utc)).clamp(0.0, whole),
            whole_days: whole,
        }
    }

    fn elapsed_working_days(self, now: DateTime<Local>, working_time: &WorkingTime) -> f64 {
        let today = now.date_naive();
        let whole_days_before_today =
            count_working_days(self.since, today.min(self.until), working_time.days);
        let today_is_working = self.contains(today) && working_time.days.contains(today.weekday());
        let today_share = if today_is_working {
            working_time.hours.elapsed_share(now.time())
        } else {
            0.0
        };
        f64::from(whole_days_before_today) + today_share
    }

    fn progress(self, now: DateTime<Local>, pacing: &SpendPacing) -> PeriodProgress {
        match pacing {
            SpendPacing::CalendarTime => self.calendar_progress(now),
            SpendPacing::WorkingTime(working_time) => PeriodProgress {
                elapsed_days: self.elapsed_working_days(now, working_time),
                whole_days: f64::from(count_working_days(
                    self.since,
                    self.until,
                    working_time.days,
                )),
            },
        }
    }

    #[must_use]
    pub fn projected_cost(
        self,
        cost: &Money,
        now: DateTime<Local>,
        budget: &SpendBudget,
    ) -> Option<Money> {
        let progress = self.progress(now, &budget.pacing);
        let share = Decimal::from_f64(progress.elapsed_share())
            .filter(|_| progress.elapsed_days >= MIN_ELAPSED_DAYS)?;
        let projected = cost.amount().checked_div(share)?;
        Some(Money::from_decimal(projected, cost.currency()))
    }

    #[must_use]
    pub fn budget_quota(
        self,
        label: &'static str,
        cost: &Money,
        budget: &SpendBudget,
        now: DateTime<Local>,
    ) -> Quota {
        let used = Ratio::new::<ratio>(cost.to_f64_lossy() / f64::from(budget.monthly_limit_usd));
        self.paced_quota(label, used, &budget.pacing, now)
    }

    #[must_use]
    pub fn paced_quota(
        self,
        label: impl Into<Cow<'static, str>>,
        used: Ratio,
        pacing: &SpendPacing,
        now: DateTime<Local>,
    ) -> Quota {
        Quota {
            label: label.into(),
            used,
            elapsed: Ratio::new::<ratio>(self.progress(now, pacing).elapsed_share()),
            starts_at: local_midnight(self.since),
            resets_at: local_midnight(self.until),
        }
    }
}

impl PeriodProgress {
    fn elapsed_share(self) -> f64 {
        if self.whole_days > 0.0 {
            self.elapsed_days / self.whole_days
        } else {
            1.0
        }
    }
}

fn month_start(day: NaiveDate) -> NaiveDate {
    day - TimeDelta::days(i64::from(day.day0()))
}

fn local_midnight(day: NaiveDate) -> DateTime<Utc> {
    let midnight = day.and_time(NaiveTime::MIN);
    Local.from_local_datetime(&midnight).earliest().map_or_else(
        || {
            let offset = Local.offset_from_utc_datetime(&midnight).local_minus_utc();
            midnight.and_utc() - TimeDelta::seconds(i64::from(offset))
        },
        |local| local.with_timezone(&Utc),
    )
}

fn days_between(from: DateTime<Utc>, to: DateTime<Utc>) -> f64 {
    (to - from).as_seconds_f64() / TimeDelta::days(1).as_seconds_f64()
}

fn count_working_days(since: NaiveDate, until: NaiveDate, working_days: WeekdaySet) -> u32 {
    since
        .iter_days()
        .take_while(|day| *day < until)
        .filter(|day| working_days.contains(day.weekday()))
        .map(|_| 1)
        .sum()
}

#[cfg(test)]
mod tests {
    use sottomano_toml_config::severity::Severity;

    use super::*;
    use crate::ai_usage_spend::spend_money::usd;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    const CALENDAR_BUDGET: SpendBudget = SpendBudget {
        pacing: SpendPacing::CalendarTime,
        monthly_limit_usd: 20,
    };

    fn local_moment(text: &str) -> DateTime<Local> {
        Local
            .from_local_datetime(&text.parse().unwrap())
            .earliest()
            .unwrap()
    }

    #[test]
    fn spending_past_the_budget_on_the_first_morning_is_critical() {
        let now = local_moment("2026-10-01T01:00:00");
        let quota = CalendarSpan::month_containing(now.date_naive()).budget_quota(
            "Monthly",
            &usd(Decimal::from(25)),
            &CALENDAR_BUDGET,
            now,
        );
        assert_eq!(quota.severity(), Severity::Critical);
    }

    #[test]
    fn calendar_elapsed_share_spans_the_real_window_across_daylight_saving() {
        let now = local_moment("2026-10-24T09:18:00");
        let quota = CalendarSpan::month_containing(now.date_naive()).budget_quota(
            "Monthly",
            &usd(Decimal::from(10)),
            &CALENDAR_BUDGET,
            now,
        );
        let window = (quota.resets_at - quota.starts_at).as_seconds_f64();
        let elapsed = (now.with_timezone(&Utc) - quota.starts_at).as_seconds_f64();
        assert!((quota.elapsed.get::<ratio>() * window - elapsed).abs() < 1.0);
    }

    #[test]
    fn december_rolls_over_into_january() {
        assert_eq!(
            CalendarSpan::month_containing(date("2026-12-15")).until,
            date("2027-01-01")
        );
        assert_eq!(
            CalendarSpan::month_containing(date("2026-02-01")).until,
            date("2026-03-01")
        );
    }
}
