use std::{borrow::Cow, cmp::Reverse};

use chrono::{DateTime, TimeDelta, Utc};
use uom::si::{f64::Ratio, ratio::ratio};

use super::{
    Accounting,
    focus_units::FocusUnit,
    recorded_spans::{FocusScope, FocusSpan},
    time_intervals::{Interval, merged},
};
use crate::data::source::ai::quota::Quota;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitTotal {
    pub unit: FocusUnit,
    pub spent: TimeDelta,
    pub budget: Option<TimeDelta>,
}

impl Accounting<'_> {
    pub fn switches_within(&self, window: Interval, now: DateTime<Utc>) -> usize {
        self.ledger
            .window_focus_spans_ended_by(now)
            .filter(|span| window.from <= span.started_at && span.started_at < window.until)
            .count()
    }

    pub fn switches_to(&self, scope: &FocusScope, window: Interval, now: DateTime<Utc>) -> usize {
        self.ledger
            .window_focus_spans_ended_by(now)
            .scan(None::<FocusSpan>, |previous, span| {
                let entered = scope.covers(&span)
                    && !previous.as_ref().is_some_and(|previous| {
                        scope.covers(previous) && previous.ended_at >= span.started_at
                    });
                *previous = Some(span.clone());
                Some(entered.then_some(span.started_at))
            })
            .flatten()
            .filter(|started_at| window.from <= *started_at && *started_at < window.until)
            .count()
    }

    fn active_parts(
        &self,
        spans: impl Iterator<Item = FocusSpan>,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Vec<Interval> {
        let active = self.active_within(window, now);
        spans
            .flat_map(|span| {
                active
                    .iter()
                    .filter_map(move |interval| interval.intersect(span.interval()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn spent(&self, scope: &FocusScope, window: Interval, now: DateTime<Utc>) -> TimeDelta {
        let covered = self
            .ledger
            .window_focus_spans_ended_by(now)
            .filter(|span| scope.covers(span));
        self.active_parts(covered, window, now)
            .into_iter()
            .map(Interval::length)
            .sum()
    }

    pub fn longest_stay_on(
        &self,
        scope: &FocusScope,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Option<Interval> {
        let covered = self
            .ledger
            .window_focus_spans_ended_by(now)
            .filter(|span| scope.covers(span));
        merged(self.active_parts(covered, window, now))
            .into_iter()
            .max_by_key(|stay| stay.length())
    }

    pub fn unit_totals(&self, window: Interval, now: DateTime<Utc>) -> Vec<UnitTotal> {
        let mut units: Vec<FocusUnit> = self
            .ledger
            .window_focus_spans_ended_by(now)
            .filter(|span| span.interval().intersect(window).is_some())
            .flat_map(|span| span.units)
            .collect();
        units.sort();
        units.dedup();
        let mut totals: Vec<UnitTotal> = units
            .into_iter()
            .map(|unit| UnitTotal {
                spent: self.spent(&FocusScope::Unit(unit.clone()), window, now),
                budget: unit.budget(self.config),
                unit,
            })
            .collect();
        totals.sort_by_key(|total| Reverse(total.spent));
        totals
    }

    pub fn unit_totals_within(
        &self,
        scope: &FocusScope,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Vec<UnitTotal> {
        let mut companions: Vec<FocusUnit> = self
            .ledger
            .window_focus_spans_ended_by(now)
            .filter(|span| scope.covers(span))
            .filter(|span| span.interval().intersect(window).is_some())
            .flat_map(|span| span.units)
            .filter(|companion| !matches!(scope, FocusScope::Unit(unit) if unit == companion))
            .collect();
        companions.sort();
        companions.dedup();
        let mut totals: Vec<UnitTotal> = companions
            .into_iter()
            .map(|companion| {
                let together = self
                    .ledger
                    .window_focus_spans_ended_by(now)
                    .filter(|span| scope.covers(span) && span.units.contains(&companion));
                UnitTotal {
                    spent: self
                        .active_parts(together, window, now)
                        .into_iter()
                        .map(Interval::length)
                        .sum(),
                    budget: companion.budget(self.config),
                    unit: companion,
                }
            })
            .collect();
        totals.sort_by_key(|total| Reverse(total.spent));
        totals
    }

    pub fn screen_budget(&self) -> Option<TimeDelta> {
        self.config
            .screen_daily_budget_minutes
            .map(|minutes| TimeDelta::minutes(i64::from(minutes)))
    }

    fn budget_quota(
        label: Cow<'static, str>,
        spent: TimeDelta,
        budget: TimeDelta,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Quota {
        let used = spent.as_seconds_f64() / budget.as_seconds_f64().max(1.0);
        Quota::within_window(
            label,
            Ratio::new::<ratio>(used),
            window.until,
            window.length(),
            now,
        )
    }

    pub fn screen_quota(&self, window: Interval, now: DateTime<Utc>) -> Option<Quota> {
        self.screen_budget().map(|budget| {
            Self::budget_quota(
                Cow::Borrowed("Screen"),
                self.screen_spent(window, now),
                budget,
                window,
                now,
            )
        })
    }

    pub fn quotas_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Quota> {
        self.unit_totals(window, now)
            .iter()
            .filter_map(|total| total.quota(window, now))
            .chain(self.screen_quota(window, now))
            .collect()
    }
}

impl UnitTotal {
    pub fn quota(&self, window: Interval, now: DateTime<Utc>) -> Option<Quota> {
        self.budget.map(|budget| {
            Accounting::budget_quota(
                Cow::Owned(self.unit.label().into_owned()),
                self.spent,
                budget,
                window,
                now,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::data::{
        severity::Severity,
        source::focus_ledger::test_steps::{
            at, config, focused, focused_in, folded, kitty, step, whole_day,
        },
    };

    #[test]
    fn switches_count_span_starts_inside_the_window() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(focused("firefox", None), 30),
            step(focused("kitty", None), 50),
        ]);
        let last_hour = Interval {
            from: at(10),
            until: at(70),
        };
        assert_eq!(
            ledger.accounted(&config).switches_within(last_hour, at(70)),
            2
        );
    }

    #[test]
    fn spent_time_is_clipped_to_the_window() {
        let config = config();
        let ledger = folded([step(focused("kitty", None), 0)]);
        let accounted = ledger.accounted(&config);
        let window = Interval {
            from: at(10),
            until: at(25),
        };
        assert_eq!(
            accounted.spent(&kitty(), window, at(40)),
            TimeDelta::minutes(15)
        );
        assert_eq!(
            accounted.screen_spent(window, at(40)),
            TimeDelta::minutes(15)
        );
    }

    #[test]
    fn a_budget_outpaced_before_the_window_ends_warns() {
        let mut config = config();
        config
            .app_daily_budget_minutes
            .insert("kitty".to_owned(), 60);
        let ledger = folded([step(focused("kitty", None), 0)]);
        let accounted = ledger.accounted(&config);
        let window = Interval {
            from: at(0),
            until: at(600),
        };
        let quotas = accounted.quotas_within(window, at(120));
        assert_eq!(quotas.len(), 1);
        assert_eq!(quotas[0].severity(), Severity::Critical);
        assert!(accounted.quotas_within(window, at(-30)).is_empty());
    }

    #[test]
    fn per_app_switches_stays_and_companions() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", Some("/code/vetrata")), 0),
            step(focused("firefox", None), 10),
            step(focused("kitty", Some("/code/other")), 20),
            step(focused("kitty", Some("/code/vetrata")), 25),
        ]);
        let accounted = ledger.accounted(&config);
        assert_eq!(accounted.switches_to(&kitty(), whole_day(), at(28)), 2);
        assert_eq!(
            accounted.longest_stay_on(&kitty(), whole_day(), at(28)),
            Some(Interval {
                from: at(0),
                until: at(10)
            })
        );
        let companions = accounted.unit_totals_within(&kitty(), whole_day(), at(28));
        assert_eq!(companions.len(), 2);
        assert_eq!(
            companions[0].unit,
            FocusUnit::Project(PathBuf::from("/code/vetrata"))
        );
        assert_eq!(companions[0].spent, TimeDelta::minutes(13));
    }

    #[test]
    fn each_window_of_one_app_is_tracked_on_its_own() {
        let config = config();
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/vetrata")), 0),
            step(focused_in(2, "kitty", Some("/code/vetrata")), 10),
        ]);
        let accounted = ledger.accounted(&config);
        let first = FocusScope::Window(1);
        let second = FocusScope::Window(2);
        assert_eq!(
            accounted.spent(&first, whole_day(), at(30)),
            TimeDelta::minutes(10)
        );
        assert_eq!(
            accounted.spent(&second, whole_day(), at(30)),
            TimeDelta::minutes(20)
        );
        assert_eq!(accounted.switches_to(&second, whole_day(), at(30)), 1);
        assert_eq!(accounted.switches_to(&kitty(), whole_day(), at(30)), 1);
        assert_eq!(ledger.project_shown_in(2), Some(Path::new("/code/vetrata")));
        assert_eq!(ledger.project_shown_in(1), None);
    }
}
