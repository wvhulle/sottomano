use std::{borrow::Cow, collections::HashMap};

use chrono::{DateTime, TimeDelta, Utc};
use uom::si::{f64::Ratio, ratio::ratio};

use super::{
    Accounting,
    focus_units::{FocusUnit, UnitIdentity},
    recorded_spans::{FocusPiece, FocusScope},
    time_intervals::{Interval, merged, overlaps_with},
};
use crate::ai::quota::Quota;

struct UnitTally<'a> {
    unit: &'a FocusUnit,
    spent: TimeDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitTotal {
    pub unit: FocusUnit,
    pub spent: TimeDelta,
    pub budget: Option<TimeDelta>,
}

impl Accounting<'_> {
    #[must_use]
    pub fn switches_within(&self, window: Interval, now: DateTime<Utc>) -> usize {
        self.ledger
            .focus_pieces_within(window, now)
            .filter(|piece| window.from <= piece.interval.from)
            .count()
    }

    #[must_use]
    pub fn switches_to(&self, scope: &FocusScope, window: Interval, now: DateTime<Utc>) -> usize {
        self.ledger
            .focus_pieces_within(window, now)
            .scan(None::<FocusPiece>, |previous, piece| {
                let entered = scope.covers(&piece)
                    && !previous.is_some_and(|previous| {
                        scope.covers(&previous) && previous.interval.until >= piece.interval.from
                    });
                *previous = Some(piece);
                Some(entered.then_some(piece.interval.from))
            })
            .flatten()
            .filter(|started_at| window.from <= *started_at)
            .count()
    }

    fn active_parts(
        &self,
        scope: &FocusScope,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Vec<Interval> {
        let active = self.ledger.active_within(window, now);
        self.ledger
            .focus_pieces_within(window, now)
            .filter(|piece| scope.covers(piece))
            .flat_map(|piece| overlaps_with(&active, piece.interval))
            .collect()
    }

    fn spent_per_unit<'p>(
        &self,
        settled: HashMap<UnitIdentity<'p>, UnitTally<'p>>,
        pieces: impl Iterator<Item = FocusPiece<'p>>,
        counted: impl Fn(&FocusUnit) -> bool,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Vec<UnitTotal> {
        let active = self.ledger.active_within(window, now);
        let spent_by_unit = pieces
            .filter(|piece| piece.interval.intersect(window).is_some())
            .fold(settled, |spent_by_unit, piece| {
                let overlap: TimeDelta = overlaps_with(&active, piece.interval)
                    .map(Interval::length)
                    .sum();
                piece.units.iter().filter(|unit| counted(unit)).fold(
                    spent_by_unit,
                    |mut spent_by_unit, unit| {
                        spent_by_unit
                            .entry(unit.identity())
                            .or_insert(UnitTally {
                                unit,
                                spent: TimeDelta::zero(),
                            })
                            .spent += overlap;
                        spent_by_unit
                    },
                )
            });
        let mut totals: Vec<UnitTotal> = spent_by_unit
            .into_values()
            .map(|tally| UnitTotal {
                spent: tally.spent,
                budget: tally.unit.budget(self.config),
                unit: tally.unit.clone(),
            })
            .collect();
        totals.sort_by(|first, second| {
            second
                .spent
                .cmp(&first.spent)
                .then_with(|| first.unit.cmp(&second.unit))
        });
        totals
    }

    pub fn spent(&self, scope: &FocusScope, window: Interval, now: DateTime<Utc>) -> TimeDelta {
        self.active_parts(scope, window, now)
            .into_iter()
            .map(Interval::length)
            .sum()
    }

    #[must_use]
    pub fn longest_stay_on(
        &self,
        scope: &FocusScope,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Option<Interval> {
        merged(self.active_parts(scope, window, now))
            .into_iter()
            .max_by_key(|stay| stay.length())
    }

    #[must_use]
    pub fn unit_totals(&self, window: Interval, now: DateTime<Utc>) -> Vec<UnitTotal> {
        let settled = self.ledger.settled_unit_totals_for(window);
        self.spent_per_unit(
            settled
                .into_iter()
                .flat_map(|settled| &settled.spent)
                .map(|(unit, spent)| {
                    (
                        unit.identity(),
                        UnitTally {
                            unit,
                            spent: *spent,
                        },
                    )
                })
                .collect(),
            self.ledger.unsettled_focus_pieces_within(
                settled.map_or(0, |settled| settled.settled_spans),
                window,
                now,
            ),
            |_| true,
            window,
            now,
        )
    }

    #[must_use]
    pub fn unit_totals_within(
        &self,
        scope: &FocusScope,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Vec<UnitTotal> {
        self.spent_per_unit(
            HashMap::new(),
            self.ledger
                .focus_pieces_within(window, now)
                .filter(|piece| scope.covers(piece)),
            |companion| !matches!(scope, FocusScope::Unit(unit) if unit == companion),
            window,
            now,
        )
    }

    #[must_use]
    pub fn screen_budget(&self) -> Option<TimeDelta> {
        self.config
            .screen_daily_budget_minutes
            .map(|minutes| TimeDelta::minutes(i64::from(minutes)))
    }

    #[must_use]
    pub fn screen_budget_status(
        &self,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Option<BudgetStatus> {
        self.screen_budget().map(|budget| {
            BudgetStatus::within(
                Cow::Borrowed("Screen"),
                self.screen_spent(window, now),
                budget,
                window,
                now,
            )
        })
    }
}

impl UnitTotal {
    #[must_use]
    pub fn budget_status(&self, window: Interval, now: DateTime<Utc>) -> Option<BudgetStatus> {
        self.budget.map(|budget| {
            BudgetStatus::within(
                Cow::Owned(self.unit.label().into_owned()),
                self.spent,
                budget,
                window,
                now,
            )
        })
    }
}

#[derive(Debug, Clone)]
pub struct BudgetStatus {
    pub budget: TimeDelta,
    pub quota: Quota,
}

impl BudgetStatus {
    fn within(
        label: Cow<'static, str>,
        spent: TimeDelta,
        budget: TimeDelta,
        window: Interval,
        now: DateTime<Utc>,
    ) -> Self {
        let used = spent.as_seconds_f64() / budget.as_seconds_f64().max(1.0);
        BudgetStatus {
            budget,
            quota: Quota::within_window(
                label,
                Ratio::new::<ratio>(used),
                window.until,
                window.length(),
                now,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use sottomano_config::severity::Severity;

    use super::*;
    use crate::focus_ledger::test_steps::{
        at, config, focused, focused_in, folded, kitty, step, whole_day,
    };

    #[test]
    fn settled_totals_plus_later_spans_match_a_full_walk() {
        let config = config();
        let walked = folded([
            step(focused("kitty", Some("/code/sottomano")), 0),
            step(focused("firefox", None), 10),
            step(focused("kitty", Some("/code/sottomano")), 25),
            step(focused("firefox", None), 40),
        ]);
        let settled = folded([
            step(focused("kitty", Some("/code/sottomano")), 0),
            step(focused("firefox", None), 10),
        ])
        .with_settled_unit_totals(whole_day(), at(10))
        .fold(focused("kitty", Some("/code/sottomano")), at(25))
        .ledger
        .fold(focused("firefox", None), at(40))
        .ledger;
        assert_eq!(
            settled.accounted(&config).unit_totals(whole_day(), at(50)),
            walked.accounted(&config).unit_totals(whole_day(), at(50))
        );
    }

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
        let budgeted = |now| {
            accounted
                .unit_totals(window, now)
                .iter()
                .filter_map(|total| total.budget_status(window, now))
                .collect::<Vec<_>>()
        };
        let statuses = budgeted(at(120));
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].quota.severity(), Severity::Critical);
        assert!(budgeted(at(-30)).is_empty());
    }

    #[test]
    fn per_app_switches_stays_and_companions() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", Some("/code/sottomano")), 0),
            step(focused("firefox", None), 10),
            step(focused("kitty", Some("/code/other")), 20),
            step(focused("kitty", Some("/code/sottomano")), 25),
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
            FocusUnit::Project(PathBuf::from("/code/sottomano"))
        );
        assert_eq!(companions[0].spent, TimeDelta::minutes(13));
    }

    #[test]
    fn each_window_of_one_app_is_tracked_on_its_own() {
        let config = config();
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/sottomano")), 0),
            step(focused_in(2, "kitty", Some("/code/sottomano")), 10),
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
        assert_eq!(
            ledger.project_shown_in(2),
            Some(Path::new("/code/sottomano"))
        );
        assert_eq!(ledger.project_shown_in(1), None);
    }
}
