use std::{borrow::Cow, collections::HashMap};

use chrono::{DateTime, TimeDelta, Utc};
use uom::si::{f64::Ratio, ratio::ratio};

use super::{
    Accounting,
    focus_units::FocusUnit,
    recorded_spans::FocusPiece,
    time_intervals::{Interval, LAST_DAY, merged, overlaps_with},
};
use crate::ai_usage_spend::usage_quota::Quota;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitTotal {
    pub unit: FocusUnit,
    pub spent: TimeDelta,
    pub budget: Option<TimeDelta>,
}

impl Accounting<'_> {
    #[must_use]
    pub fn switches_within(&self, period: Interval, now: DateTime<Utc>) -> usize {
        self.ledger
            .focus_pieces_within(period, now)
            .filter(|piece| period.from <= piece.interval.from)
            .count()
    }

    #[must_use]
    pub fn switches_to(&self, window_id: u64, period: Interval, now: DateTime<Utc>) -> usize {
        self.ledger
            .focus_pieces_within(period, now)
            .scan(None::<FocusPiece>, |previous, piece| {
                let entered = piece.window == window_id
                    && !previous.is_some_and(|previous| {
                        previous.window == window_id
                            && previous.interval.until >= piece.interval.from
                    });
                *previous = Some(piece);
                Some(entered.then_some(piece.interval.from))
            })
            .flatten()
            .filter(|started_at| period.from <= *started_at)
            .count()
    }

    fn active_parts_on(
        &self,
        window_id: u64,
        period: Interval,
        now: DateTime<Utc>,
    ) -> Vec<Interval> {
        let active = self.ledger.active_within(period, now);
        self.ledger
            .focus_pieces_within(period, now)
            .filter(|piece| piece.window == window_id)
            .flat_map(|piece| overlaps_with(&active, piece.interval))
            .collect()
    }

    fn totals_of_pieces<'pieces>(
        &self,
        pieces: impl Iterator<Item = FocusPiece<'pieces>>,
        period: Interval,
        now: DateTime<Utc>,
    ) -> Vec<UnitTotal> {
        let active = self.ledger.active_within(period, now);
        let spent_by_unit = pieces
            .filter(|piece| piece.interval.intersect(period).is_some())
            .fold(
                HashMap::<&FocusUnit, TimeDelta>::new(),
                |mut spent_by_unit, piece| {
                    let overlap: TimeDelta = overlaps_with(&active, piece.interval)
                        .map(Interval::length)
                        .sum();
                    for unit in piece.units {
                        *spent_by_unit.entry(unit).or_insert_with(TimeDelta::zero) += overlap;
                    }
                    spent_by_unit
                },
            );
        let mut totals: Vec<UnitTotal> = spent_by_unit
            .into_iter()
            .map(|(unit, spent)| UnitTotal {
                spent,
                budget: unit.budget(self.config),
                unit: unit.clone(),
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

    pub fn spent_on(&self, window_id: u64, period: Interval, now: DateTime<Utc>) -> TimeDelta {
        self.active_parts_on(window_id, period, now)
            .into_iter()
            .map(Interval::length)
            .sum()
    }

    #[must_use]
    pub fn longest_stay_on(
        &self,
        window_id: u64,
        period: Interval,
        now: DateTime<Utc>,
    ) -> Option<Interval> {
        merged(self.active_parts_on(window_id, period, now))
            .into_iter()
            .max_by_key(|stay| stay.length())
    }

    #[must_use]
    pub fn unit_totals(&self, period: Interval, now: DateTime<Utc>) -> Vec<UnitTotal> {
        self.totals_of_pieces(self.ledger.focus_pieces_within(period, now), period, now)
    }

    #[must_use]
    pub fn unit_totals_on(
        &self,
        window_id: u64,
        period: Interval,
        now: DateTime<Utc>,
    ) -> Vec<UnitTotal> {
        self.totals_of_pieces(
            self.ledger
                .focus_pieces_within(period, now)
                .filter(|piece| piece.window == window_id),
            period,
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
    pub fn screen_budget_status(&self, now: DateTime<Utc>) -> Option<BudgetStatus> {
        self.screen_budget().map(|budget| {
            BudgetStatus::within_last_day(
                Cow::Borrowed("Screen"),
                self.screen_spent(LAST_DAY.ending_at(now), now),
                budget,
                now,
            )
        })
    }
}

impl UnitTotal {
    #[must_use]
    pub fn budget_status(&self, now: DateTime<Utc>) -> Option<BudgetStatus> {
        self.budget.map(|budget| {
            BudgetStatus::within_last_day(
                Cow::Owned(self.unit.label().into_owned()),
                self.spent,
                budget,
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
    fn within_last_day(
        label: Cow<'static, str>,
        spent: TimeDelta,
        budget: TimeDelta,
        now: DateTime<Utc>,
    ) -> Self {
        let used = spent.as_seconds_f64() / budget.as_seconds_f64().max(1.0);
        BudgetStatus {
            budget,
            quota: Quota::within_window(
                label,
                Ratio::new::<ratio>(used),
                now,
                LAST_DAY.length,
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
    use crate::login_session_presence::focus_time_ledger::test_steps::{
        at, config, focused, focused_in, folded, step, whole_day,
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
        let period = Interval {
            from: at(10),
            until: at(25),
        };
        assert_eq!(
            accounted.spent_on(1, period, at(40)),
            TimeDelta::minutes(15)
        );
        assert_eq!(
            accounted.screen_spent(period, at(40)),
            TimeDelta::minutes(15)
        );
    }

    #[test]
    fn a_budget_exceeded_in_the_last_day_turns_critical() {
        let mut config = config();
        config
            .app_daily_budget_minutes
            .insert("kitty".to_owned(), 60);
        let ledger = folded([step(focused("kitty", None), 0)]);
        let accounted = ledger.accounted(&config);
        let severity_at = |now| {
            accounted
                .unit_totals(LAST_DAY.ending_at(now), now)
                .iter()
                .filter_map(|total| total.budget_status(now))
                .map(|status| status.quota.severity())
                .collect::<Vec<_>>()
        };
        assert_eq!(severity_at(at(30)), [Severity::Normal]);
        assert_eq!(severity_at(at(120)), [Severity::Critical]);
        assert!(severity_at(at(-30)).is_empty());
    }

    #[test]
    fn per_window_switches_stays_and_projects() {
        let config = config();
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/sottomano")), 0),
            step(focused_in(2, "firefox", None), 10),
            step(focused_in(1, "kitty", Some("/code/other")), 20),
            step(focused_in(1, "kitty", Some("/code/sottomano")), 25),
        ]);
        let accounted = ledger.accounted(&config);
        assert_eq!(accounted.switches_to(1, whole_day(), at(28)), 2);
        assert_eq!(
            accounted.longest_stay_on(1, whole_day(), at(28)),
            Some(Interval {
                from: at(0),
                until: at(10)
            })
        );
        let totals = accounted.unit_totals_on(1, whole_day(), at(28));
        assert_eq!(totals[0].unit, FocusUnit::App("kitty".to_owned()));
        assert_eq!(totals[0].spent, TimeDelta::minutes(18));
        assert_eq!(
            totals[1].unit,
            FocusUnit::Project(PathBuf::from("/code/sottomano"))
        );
        assert_eq!(totals[1].spent, TimeDelta::minutes(13));
    }

    #[test]
    fn each_window_of_one_app_is_tracked_on_its_own() {
        let config = config();
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/sottomano")), 0),
            step(focused_in(2, "kitty", Some("/code/sottomano")), 10),
        ]);
        let accounted = ledger.accounted(&config);
        assert_eq!(
            accounted.spent_on(1, whole_day(), at(30)),
            TimeDelta::minutes(10)
        );
        assert_eq!(
            accounted.spent_on(2, whole_day(), at(30)),
            TimeDelta::minutes(20)
        );
        assert_eq!(accounted.switches_to(2, whole_day(), at(30)), 1);
        assert_eq!(
            ledger.project_shown_in(2),
            Some(Path::new("/code/sottomano"))
        );
        assert_eq!(ledger.project_shown_in(1), None);
    }
}
