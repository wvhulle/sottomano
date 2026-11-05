use std::mem::replace;

use chrono::{DateTime, TimeDelta, Utc};

use super::{
    FocusLedger,
    focus_identity::Focused,
    focus_units::FocusUnit,
    recorded_spans::{FocusPiece, FocusSpan, SpanWrite},
    time_intervals::Interval,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OngoingWindowFocus {
    FocusedWindowSpan(FocusSpan),
    NoWindowFocused,
}

impl FocusLedger {
    pub(super) fn focus_window(
        &mut self,
        focused: &Focused,
        now: DateTime<Utc>,
        shortest_counted_focus: TimeDelta,
    ) {
        let next_span = FocusSpan {
            window: focused.window,
            units: focused.units(),
            started_at: now,
            ended_at: now,
        };
        if self
            .focused_window_span()
            .is_some_and(|ongoing| ongoing.continues(&next_span))
        {
            return;
        }
        self.finish_ongoing_window_focus(
            OngoingWindowFocus::NoWindowFocused,
            shortest_counted_focus,
        );
        let resumed_span = self.finished_window_focus_spans.pop_if(|last| {
            last.continues(&next_span) && now - last.ended_at < shortest_counted_focus
        });
        self.ongoing_window_focus =
            OngoingWindowFocus::FocusedWindowSpan(resumed_span.unwrap_or(next_span));
    }

    pub(super) fn lose_window_focus(&mut self, shortest_counted_focus: TimeDelta) {
        self.finish_ongoing_window_focus(
            OngoingWindowFocus::NoWindowFocused,
            shortest_counted_focus,
        );
    }

    fn finish_ongoing_window_focus(
        &mut self,
        next: OngoingWindowFocus,
        shortest_counted_focus: TimeDelta,
    ) {
        if let OngoingWindowFocus::FocusedWindowSpan(finished) =
            replace(&mut self.ongoing_window_focus, next)
        {
            self.keep_finished_window_focus(finished, shortest_counted_focus);
        }
    }

    pub(super) fn keep_finished_window_focus(
        &mut self,
        finished: FocusSpan,
        shortest_counted_focus: TimeDelta,
    ) {
        if finished.ended_at - finished.started_at < shortest_counted_focus {
            return;
        }
        match self.finished_window_focus_spans.last_mut() {
            Some(last)
                if last.continues(&finished)
                    && finished.started_at - last.ended_at < shortest_counted_focus =>
            {
                last.ended_at = last.ended_at.max(finished.ended_at);
            }
            _ => self.finished_window_focus_spans.push(finished),
        }
    }

    pub(super) fn ongoing_focus_write(
        &mut self,
        now: DateTime<Utc>,
        shortest_counted_focus: TimeDelta,
    ) -> Option<SpanWrite> {
        match &mut self.ongoing_window_focus {
            OngoingWindowFocus::FocusedWindowSpan(span) => {
                span.ended_at = now;
                (now - span.started_at >= shortest_counted_focus)
                    .then(|| SpanWrite::RecordWindowFocusSpan(span.clone()))
            }
            OngoingWindowFocus::NoWindowFocused => None,
        }
    }

    pub(super) fn focused_window_span(&self) -> Option<&FocusSpan> {
        match &self.ongoing_window_focus {
            OngoingWindowFocus::FocusedWindowSpan(span) => Some(span),
            OngoingWindowFocus::NoWindowFocused => None,
        }
    }

    #[must_use]
    pub fn focused_units(&self) -> &[FocusUnit] {
        self.focused_window_span()
            .map_or(&[], |span| span.units.as_slice())
    }

    pub(super) fn focus_pieces_within(
        &self,
        window: Interval,
        now: DateTime<Utc>,
    ) -> impl Iterator<Item = FocusPiece<'_>> + '_ {
        let first_reaching_window = self
            .finished_window_focus_spans
            .partition_point(|span| span.ended_at < window.from);
        self.finished_window_focus_spans[first_reaching_window..]
            .iter()
            .map(FocusSpan::piece)
            .chain(self.focused_window_span().map(|span| FocusPiece {
                interval: Interval {
                    from: span.started_at,
                    until: now,
                },
                ..span.piece()
            }))
            .take_while(move |piece| piece.interval.from < window.until)
    }
}
