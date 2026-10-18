mod away_reasons;
mod focus_identity;
mod focus_units;
mod presence_accounting;
mod presence_timeline;
mod recorded_spans;
mod settled_unit_totals;
#[cfg(test)]
mod test_steps;
mod time_intervals;
mod unit_accounting;
mod window_focus_timeline;

use std::path::Path;

use chrono::{DateTime, Utc};
use vetrata_config::widget_groups::focus_time::FocusTimeConfig;

pub use self::{
    away_reasons::{AwayReasonEdge, ScreenEdge},
    focus_identity::FocusIdentity,
    focus_units::{FocusUnit, StoredFacet},
    presence_accounting::{FocusRun, ScreenRun},
    recorded_spans::{FocusScope, FocusSpan, LoadedSpans, PresenceSpan, SpanWrite},
    time_intervals::{Interval, WakingWindow},
    unit_accounting::{BudgetStatus, UnitTotal},
    window_focus_timeline::WindowFocusChange,
};
#[cfg(test)]
pub use self::{focus_identity::Focused, focus_units::Presence};
use self::{
    presence_timeline::PresenceTimeline,
    recorded_spans::{FocusPiece, TimelineUpdate},
    settled_unit_totals::SettledUnitTotals,
    window_focus_timeline::WindowFocusTimeline,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusSignal {
    WindowFocusChanged(WindowFocusChange),
    AwayReasonChanged(AwayReasonEdge),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusLedger {
    window_focus_timeline: WindowFocusTimeline,
    presence_timeline: PresenceTimeline,
    settled_unit_totals: Option<SettledUnitTotals>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folded {
    pub ledger: FocusLedger,
    pub span_writes: Vec<SpanWrite>,
}

#[derive(Debug, Clone, Copy)]
pub struct Accounting<'a> {
    pub ledger: &'a FocusLedger,
    pub config: &'a FocusTimeConfig,
}

impl FocusLedger {
    #[must_use]
    pub fn started_at(now: DateTime<Utc>) -> Self {
        Self {
            window_focus_timeline: WindowFocusTimeline::no_window_focused_since(now),
            presence_timeline: PresenceTimeline::active_since(now),
            settled_unit_totals: None,
        }
    }

    #[must_use]
    pub fn with_history(self, loaded: LoadedSpans) -> Self {
        Self {
            window_focus_timeline: self
                .window_focus_timeline
                .with_loaded_history(loaded.window_focus_spans),
            presence_timeline: self
                .presence_timeline
                .with_loaded_history(loaded.presence_spans),
            settled_unit_totals: None,
        }
    }

    #[must_use]
    pub fn with_settled_unit_totals(mut self, window: Interval, now: DateTime<Utc>) -> Self {
        let active = self.active_within(window, now);
        let carried = match self.settled_unit_totals.take() {
            Some(settled) if settled.window == window => settled,
            Some(_) | None => SettledUnitTotals::within(window),
        };
        Self {
            settled_unit_totals: Some(carried.settling(
                self.window_focus_timeline.finished_window_focus_spans(),
                &active,
            )),
            ..self
        }
    }

    #[must_use]
    pub fn accounted<'a>(&'a self, config: &'a FocusTimeConfig) -> Accounting<'a> {
        Accounting {
            ledger: self,
            config,
        }
    }

    #[must_use]
    pub fn focused_units(&self) -> &[FocusUnit] {
        self.window_focus_timeline.focused_units()
    }

    #[must_use]
    pub fn project_shown_in(&self, window: u64) -> Option<&Path> {
        self.window_focus_timeline
            .focused_window_span()
            .filter(|span| span.window == window)?
            .units
            .iter()
            .find_map(|unit| match unit {
                FocusUnit::Project(root) => Some(root.as_path()),
                FocusUnit::App(_) => None,
            })
    }

    #[must_use]
    pub fn screen_off_since(&self) -> Option<DateTime<Utc>> {
        self.presence_timeline.screen_off_since()
    }

    #[must_use]
    pub fn fold(self, signal: FocusSignal, now: DateTime<Utc>) -> Folded {
        match signal {
            FocusSignal::WindowFocusChanged(change) => {
                let presence = self.presence_timeline.ongoing_presence_extended_to(now);
                let window_focus = self
                    .window_focus_timeline
                    .following_window_focus_change(change, now);
                Self::folded_from(window_focus, presence, self.settled_unit_totals)
            }
            FocusSignal::AwayReasonChanged(edge) => {
                let window_focus = self
                    .window_focus_timeline
                    .ongoing_window_focus_extended_to(now);
                let presence = self
                    .presence_timeline
                    .reacting_to_away_reason_edge(edge, now);
                Self::folded_from(window_focus, presence, self.settled_unit_totals)
            }
        }
    }

    fn folded_from(
        window_focus: TimelineUpdate<WindowFocusTimeline>,
        presence: TimelineUpdate<PresenceTimeline>,
        settled_unit_totals: Option<SettledUnitTotals>,
    ) -> Folded {
        Folded {
            span_writes: window_focus
                .span_writes
                .into_iter()
                .chain(presence.span_writes)
                .collect(),
            ledger: Self {
                window_focus_timeline: window_focus.timeline,
                presence_timeline: presence.timeline,
                settled_unit_totals,
            },
        }
    }

    fn settled_unit_totals_for(&self, window: Interval) -> Option<&SettledUnitTotals> {
        self.settled_unit_totals
            .as_ref()
            .filter(|settled| settled.window == window)
    }

    fn active_intervals(&self, now: DateTime<Utc>) -> Vec<Interval> {
        self.presence_timeline.active_intervals(now)
    }

    fn active_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Interval> {
        self.active_intervals(now)
            .into_iter()
            .filter_map(|interval| interval.intersect(window))
            .collect()
    }

    fn presence_spans_ended_by(
        &self,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = PresenceSpan> + '_ {
        self.presence_timeline.presence_spans_ended_by(now)
    }

    fn focus_pieces_within(
        &self,
        window: Interval,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = FocusPiece<'_>> + '_ {
        self.window_focus_timeline.pieces_within(window, now, 0)
    }

    fn unsettled_focus_pieces_within(
        &self,
        settled_spans: usize,
        window: Interval,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = FocusPiece<'_>> + '_ {
        self.window_focus_timeline
            .pieces_within(window, now, settled_spans)
    }

    fn first_presence_started_at(&self) -> DateTime<Utc> {
        self.presence_timeline.first_presence_started_at()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        test_steps::{at, config, focused, focused_in, folded, step},
        *,
    };

    fn recorded_window_focus_spans(span_writes: &[SpanWrite]) -> Vec<&FocusSpan> {
        span_writes
            .iter()
            .filter_map(|write| match write {
                SpanWrite::RecordWindowFocusSpan(span) => Some(span),
                SpanWrite::RecordPresenceSpan(_) => None,
            })
            .collect()
    }

    #[test]
    fn switching_focus_finishes_a_span_and_records_the_next() {
        let Folded {
            ledger,
            span_writes,
        } = FocusLedger::started_at(at(0)).fold(focused("kitty", Some("/code/vetrata")), at(0));
        assert_eq!(recorded_window_focus_spans(&span_writes).len(), 1);

        let Folded {
            ledger,
            span_writes,
        } = ledger.fold(focused("firefox", None), at(10));
        let recorded = recorded_window_focus_spans(&span_writes);
        assert_eq!(recorded[0].started_at, at(0));
        assert_eq!(recorded[0].ended_at, at(10));
        assert_eq!(recorded[1].started_at, at(10));
        let finished = ledger.window_focus_timeline.finished_window_focus_spans();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].ended_at, at(10));
        assert_eq!(
            ledger.focused_units(),
            [FocusUnit::App("firefox".to_owned())]
        );
        let config = config();
        assert_eq!(ledger.accounted(&config).runs(at(12))[0].since, at(10));
    }

    #[test]
    fn losing_window_focus_finishes_the_span_without_presence_changing() {
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(
                FocusSignal::WindowFocusChanged(WindowFocusChange::NoWindowFocused),
                10,
            ),
        ]);
        assert!(ledger.focused_units().is_empty());
        assert_eq!(
            ledger.window_focus_timeline.finished_window_focus_spans()[0].ended_at,
            at(10)
        );
        let config = config();
        assert_eq!(
            ledger.accounted(&config).screen_run(at(20)),
            Some(ScreenRun { since: at(0) })
        );
    }

    #[test]
    fn another_window_of_the_same_app_and_project_opens_a_new_span() {
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/vetrata")), 0),
            step(focused_in(1, "kitty", Some("/code/vetrata")), 5),
            step(focused_in(2, "kitty", Some("/code/vetrata")), 10),
        ]);
        let finished = ledger.window_focus_timeline.finished_window_focus_spans();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].window, 1);
        assert_eq!(
            ledger
                .window_focus_timeline
                .focused_window_span()
                .unwrap()
                .window,
            2
        );
    }
}
