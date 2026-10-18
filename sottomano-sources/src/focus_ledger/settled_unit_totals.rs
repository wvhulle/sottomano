use std::collections::HashMap;

use chrono::TimeDelta;

use super::{
    focus_units::FocusUnit,
    recorded_spans::FocusSpan,
    time_intervals::{Interval, overlaps_with},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SettledUnitTotals {
    pub window: Interval,
    pub settled_spans: usize,
    pub spent: HashMap<FocusUnit, TimeDelta>,
}

impl SettledUnitTotals {
    pub(super) fn within(window: Interval) -> Self {
        Self {
            window,
            settled_spans: 0,
            spent: HashMap::new(),
        }
    }

    pub(super) fn settling(self, finished: &[FocusSpan], active: &[Interval]) -> Self {
        let spent = finished[self.settled_spans..]
            .iter()
            .filter(|span| span.interval().intersect(self.window).is_some())
            .fold(self.spent, |spent, span| {
                let overlap: TimeDelta = overlaps_with(active, span.interval())
                    .map(Interval::length)
                    .sum();
                span.units.iter().fold(spent, |mut spent, unit| {
                    *spent.entry(unit.clone()).or_insert_with(TimeDelta::zero) += overlap;
                    spent
                })
            });
        Self {
            window: self.window,
            settled_spans: finished.len(),
            spent,
        }
    }
}
