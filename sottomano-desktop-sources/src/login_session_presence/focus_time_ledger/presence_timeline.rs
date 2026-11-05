use std::mem::replace;

use chrono::{DateTime, Utc};

use super::{
    FocusLedger, FocusSignal,
    focus_units::Presence,
    recorded_spans::{PresenceSpan, SpanWrite},
    time_intervals::{Interval, merged},
};

impl FocusLedger {
    pub(super) fn follow_away_reason(
        &mut self,
        signal: &FocusSignal,
        now: DateTime<Utc>,
    ) -> Option<SpanWrite> {
        self.latest_away_reasons.note_signal(signal);
        let presence = self.latest_away_reasons.most_away_presence();
        if presence == self.ongoing_presence_span.state {
            return None;
        }
        let next_presence_span = PresenceSpan {
            state: presence,
            started_at: now,
            ended_at: now,
        };
        let finished_presence_span =
            replace(&mut self.ongoing_presence_span, next_presence_span.clone());
        self.finished_presence_spans.push(finished_presence_span);
        Some(SpanWrite::RecordPresenceSpan(next_presence_span))
    }

    pub(super) fn presence_spans_ended_by(
        &self,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = PresenceSpan> + '_ {
        self.finished_presence_spans
            .iter()
            .cloned()
            .chain([self.ongoing_presence_span.clone().ending_at(now)])
    }

    pub(super) fn active_intervals(&self, now: DateTime<Utc>) -> Vec<Interval> {
        merged(
            self.presence_spans_ended_by(now)
                .filter(|span| span.state == Presence::Active)
                .map(|span| span.interval())
                .collect(),
        )
    }

    pub(super) fn active_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Interval> {
        self.active_intervals(now)
            .into_iter()
            .filter_map(|interval| interval.intersect(window))
            .collect()
    }

    pub(super) fn first_presence_started_at(&self) -> DateTime<Utc> {
        self.finished_presence_spans
            .first()
            .unwrap_or(&self.ongoing_presence_span)
            .started_at
    }

    #[must_use]
    pub fn screen_off_since(&self) -> Option<DateTime<Utc>> {
        (self.ongoing_presence_span.state == Presence::ScreenOff)
            .then_some(self.ongoing_presence_span.started_at)
    }
}
