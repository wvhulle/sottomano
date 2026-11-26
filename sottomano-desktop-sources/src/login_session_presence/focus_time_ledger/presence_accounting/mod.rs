use chrono::{DateTime, TimeDelta, Utc};

use super::{
    Accounting,
    focus_units::{FocusUnit, Presence},
    time_intervals::{Interval, gaps_between, overlaps_with},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusRun {
    pub unit: FocusUnit,
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRun {
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakCause {
    LongIdle,
    ScreenOff,
    Asleep,
}

impl BreakCause {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            BreakCause::LongIdle => "long idle",
            BreakCause::ScreenOff => "screen off",
            BreakCause::Asleep => "asleep",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakenBreak {
    pub interval: Interval,
    pub cause: BreakCause,
}

struct AwayIntervals {
    asleep: Vec<Interval>,
    screen_off: Vec<Interval>,
}

impl Accounting<'_> {
    fn idle_after(&self) -> TimeDelta {
        self.config.idle_after
    }

    fn intervals_in_state(&self, state: Presence, now: DateTime<Utc>) -> Vec<Interval> {
        self.ledger
            .presence_spans_ended_by(now)
            .filter(|span| span.state == state)
            .map(|span| span.interval())
            .collect()
    }

    fn away_intervals(&self, now: DateTime<Utc>) -> AwayIntervals {
        AwayIntervals {
            asleep: self.intervals_in_state(Presence::Asleep, now),
            screen_off: self.intervals_in_state(Presence::ScreenOff, now),
        }
    }

    fn break_cause(&self, gap: Interval, away: &AwayIntervals) -> Option<BreakCause> {
        if overlaps_with(&away.asleep, gap).next().is_some() {
            Some(BreakCause::Asleep)
        } else if overlaps_with(&away.screen_off, gap).next().is_some() {
            Some(BreakCause::ScreenOff)
        } else {
            (gap.length() >= self.idle_after()).then_some(BreakCause::LongIdle)
        }
    }

    fn taken_breaks(&self, now: DateTime<Utc>) -> Vec<TakenBreak> {
        let away = self.away_intervals(now);
        gaps_between(
            &self.ledger.active_intervals(now),
            self.ledger.first_presence_started_at(),
            now,
        )
        .into_iter()
        .filter_map(|gap| {
            self.break_cause(gap, &away).map(|cause| TakenBreak {
                interval: gap,
                cause,
            })
        })
        .collect()
    }

    fn breaks(&self, now: DateTime<Utc>) -> Vec<Interval> {
        self.taken_breaks(now)
            .into_iter()
            .map(|taken| taken.interval)
            .collect()
    }

    #[must_use]
    pub fn taken_breaks_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<TakenBreak> {
        self.taken_breaks(now)
            .into_iter()
            .filter(|taken| (window.from..window.until).contains(&taken.interval.from))
            .collect()
    }

    #[must_use]
    pub fn last_break(&self, now: DateTime<Utc>) -> Option<Interval> {
        self.breaks(now).into_iter().last()
    }

    #[must_use]
    pub fn screen_run(&self, now: DateTime<Utc>) -> Option<ScreenRun> {
        match self.last_break(now) {
            Some(gap) if gap.until >= now => None,
            Some(gap) => Some(ScreenRun { since: gap.until }),
            None => Some(ScreenRun {
                since: self.ledger.first_presence_started_at(),
            }),
        }
    }

    #[must_use]
    pub fn runs(&self, now: DateTime<Utc>) -> Vec<FocusRun> {
        let floor = self
            .last_break(now)
            .map_or(DateTime::<Utc>::MIN_UTC, |gap| gap.until);
        let Some(focused_window_span) = self.ledger.focused_window_span() else {
            return Vec::new();
        };
        focused_window_span
            .units
            .iter()
            .map(|unit| {
                let chain_start = self
                    .ledger
                    .finished_window_focus_spans
                    .iter()
                    .rev()
                    .scan(focused_window_span.started_at, |since, span| {
                        (*since <= span.ended_at && span.units.contains(unit)).then(|| {
                            *since = span.started_at;
                            *since
                        })
                    })
                    .last()
                    .unwrap_or(focused_window_span.started_at);
                FocusRun {
                    unit: unit.clone(),
                    since: chain_start.max(floor),
                }
            })
            .collect()
    }

    #[must_use]
    pub fn dominant_run(&self, now: DateTime<Utc>) -> Option<FocusRun> {
        self.runs(now).into_iter().min_by_key(|run| run.since)
    }

    #[must_use]
    pub fn longest_session_within(&self, period: Interval, now: DateTime<Utc>) -> Option<Interval> {
        gaps_between(
            &self.breaks(now),
            self.ledger.first_presence_started_at(),
            now,
        )
        .into_iter()
        .filter_map(|session| session.intersect(period))
        .max_by_key(|session| session.length())
    }

    #[must_use]
    pub fn breaks_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Interval> {
        self.taken_breaks_within(window, now)
            .into_iter()
            .map(|taken| taken.interval)
            .collect()
    }

    pub fn screen_spent(&self, window: Interval, now: DateTime<Utc>) -> TimeDelta {
        self.ledger
            .active_within(window, now)
            .into_iter()
            .map(Interval::length)
            .sum()
    }
}

#[cfg(test)]
mod presence_accounting_tests;
