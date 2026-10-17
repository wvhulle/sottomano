use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc, Weekday};
use rust_decimal::{Decimal, prelude::FromPrimitive};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    config::{Config, calendar::WorkingHours},
    data::{money::Money, source::ai::quota::Quota},
};

const MIN_ELAPSED_WORKING_DAYS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpendBudget {
    pub working_days: Vec<Weekday>,
    pub working_hours: WorkingHours,
    pub monthly_limit_usd: u32,
}

impl SpendBudget {
    pub fn new(config: &Config, monthly_limit_usd: u32) -> Self {
        Self {
            working_days: config.calendar.working_days.clone(),
            working_hours: config.calendar.working_hours.clone(),
            monthly_limit_usd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarSpan {
    pub since: NaiveDate,
    pub until: NaiveDate,
}

impl CalendarSpan {
    pub fn day(day: NaiveDate) -> Self {
        Self {
            since: day,
            until: day + TimeDelta::days(1),
        }
    }

    pub fn week_containing(day: NaiveDate) -> Self {
        let since = day - TimeDelta::days(i64::from(day.weekday().num_days_from_monday()));
        Self {
            since,
            until: since + TimeDelta::weeks(1),
        }
    }

    pub fn month_containing(day: NaiveDate) -> Self {
        let since = month_start(day);
        Self {
            since,
            until: month_start(since + TimeDelta::days(31)),
        }
    }

    pub fn contains(self, day: NaiveDate) -> bool {
        (self.since..self.until).contains(&day)
    }

    pub fn starts_at(self) -> Option<DateTime<Utc>> {
        local_midnight(self.since)
    }

    fn elapsed_working_days(self, now: DateTime<Local>, budget: &SpendBudget) -> f64 {
        let today = now.date_naive();
        let whole_days_before_today =
            count_working_days(self.since, today.min(self.until), &budget.working_days);
        let today_is_working =
            self.contains(today) && budget.working_days.contains(&today.weekday());
        let today_share = if today_is_working {
            budget.working_hours.elapsed_share(now.time())
        } else {
            0.0
        };
        f64::from(whole_days_before_today) + today_share
    }

    fn elapsed_working_share(self, now: DateTime<Local>, budget: &SpendBudget) -> Option<f64> {
        let elapsed = self.elapsed_working_days(now, budget);
        let whole = count_working_days(self.since, self.until, &budget.working_days);
        (elapsed >= MIN_ELAPSED_WORKING_DAYS && whole > 0).then(|| elapsed / f64::from(whole))
    }

    pub fn projected_cost(
        self,
        cost: &Money,
        now: DateTime<Local>,
        budget: &SpendBudget,
    ) -> Option<Money> {
        let share = Decimal::from_f64(self.elapsed_working_share(now, budget)?)?;
        let projected = cost.amount().checked_div(share)?;
        Some(Money::from_decimal(projected, cost.currency()))
    }

    pub fn budget_quota(
        self,
        label: &'static str,
        cost: &Money,
        budget: &SpendBudget,
        now: DateTime<Local>,
    ) -> Option<Quota> {
        let elapsed = self.elapsed_working_share(now, budget)?;
        Some(Quota {
            label: label.into(),
            used: Ratio::new::<ratio>(cost.to_f64_lossy() / f64::from(budget.monthly_limit_usd)),
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

fn count_working_days(since: NaiveDate, until: NaiveDate, working_days: &[Weekday]) -> u32 {
    since
        .iter_days()
        .take_while(|day| *day < until)
        .filter(|day| working_days.contains(&day.weekday()))
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
