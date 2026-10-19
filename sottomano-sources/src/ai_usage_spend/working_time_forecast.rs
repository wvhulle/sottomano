use std::borrow::Cow;

use chrono::{
    DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc, WeekdaySet,
};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use sottomano_config::{Config, calendar::WorkingHours};
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
    pub fn starts_at(self) -> Option<DateTime<Utc>> {
        local_midnight(self.since)
    }

    fn calendar_days(self) -> f64 {
        (self.until - self.since).as_seconds_f64() / TimeDelta::days(1).as_seconds_f64()
    }

    fn elapsed_calendar_days(self, now: DateTime<Local>) -> f64 {
        let elapsed = now.naive_local() - self.since.and_time(NaiveTime::MIN);
        (elapsed.as_seconds_f64() / TimeDelta::days(1).as_seconds_f64())
            .clamp(0.0, self.calendar_days())
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

    fn elapsed_share(self, now: DateTime<Local>, pacing: &SpendPacing) -> Option<f64> {
        let (elapsed, whole) = match pacing {
            SpendPacing::CalendarTime => (self.elapsed_calendar_days(now), self.calendar_days()),
            SpendPacing::WorkingTime(working_time) => (
                self.elapsed_working_days(now, working_time),
                f64::from(count_working_days(
                    self.since,
                    self.until,
                    working_time.days,
                )),
            ),
        };
        (elapsed >= MIN_ELAPSED_DAYS && whole > 0.0).then(|| elapsed / whole)
    }

    #[must_use]
    pub fn projected_cost(
        self,
        cost: &Money,
        now: DateTime<Local>,
        budget: &SpendBudget,
    ) -> Option<Money> {
        let share = Decimal::from_f64(self.elapsed_share(now, &budget.pacing)?)?;
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
    ) -> Option<Quota> {
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
    ) -> Option<Quota> {
        let elapsed = self.elapsed_share(now, pacing)?;
        Some(Quota {
            label: label.into(),
            used,
            elapsed: Ratio::new::<ratio>(elapsed),
            starts_at: local_midnight(self.since)?,
            resets_at: local_midnight(self.until)?,
        })
    }
}

fn month_start(day: NaiveDate) -> NaiveDate {
    day - TimeDelta::days(i64::from(day.day0()))
}

fn local_midnight(day: NaiveDate) -> Option<DateTime<Utc>> {
    Local
        .from_local_datetime(&day.and_time(NaiveTime::MIN))
        .earliest()
        .map(|midnight| midnight.with_timezone(&Utc))
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
    use super::*;

    fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
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
