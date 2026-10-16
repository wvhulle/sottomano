use chrono::{DateTime, Utc};

use super::{
    focus_identity::Focused,
    focus_units::FocusUnit,
    recorded_spans::{FocusPiece, FocusSpan, SpanWrite, TimelineUpdate},
    time_intervals::Interval,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum WindowFocusChange {
    WindowGainedFocus(Focused),
    #[default]
    NoWindowFocused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OngoingWindowFocus {
    FocusedWindowSpan(FocusSpan),
    NoWindowFocusedSince(DateTime<Utc>),
}

impl OngoingWindowFocus {
    fn since(&self) -> DateTime<Utc> {
        match self {
            OngoingWindowFocus::FocusedWindowSpan(span) => span.started_at,
            OngoingWindowFocus::NoWindowFocusedSince(since) => *since,
        }
    }

    fn focused_window_span(&self) -> Option<&FocusSpan> {
        match self {
            OngoingWindowFocus::FocusedWindowSpan(span) => Some(span),
            OngoingWindowFocus::NoWindowFocusedSince(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFocusTimeline {
    finished_window_focus_spans: Vec<FocusSpan>,
    ongoing_window_focus: OngoingWindowFocus,
}

impl WindowFocusTimeline {
    pub(super) fn no_window_focused_since(since: DateTime<Utc>) -> Self {
        Self {
            finished_window_focus_spans: Vec::new(),
            ongoing_window_focus: OngoingWindowFocus::NoWindowFocusedSince(since),
        }
    }

    #[must_use]
    pub(super) fn with_loaded_history(self, loaded_window_focus_spans: Vec<FocusSpan>) -> Self {
        let ongoing_since = self.ongoing_window_focus.since();
        Self {
            finished_window_focus_spans: loaded_window_focus_spans
                .into_iter()
                .filter(|loaded| loaded.started_at < ongoing_since)
                .chain(self.finished_window_focus_spans)
                .collect(),
            ..self
        }
    }

    pub(super) fn ongoing_window_focus_extended_to(
        self,
        now: DateTime<Utc>,
    ) -> TimelineUpdate<Self> {
        match self.ongoing_window_focus {
            OngoingWindowFocus::FocusedWindowSpan(span) => {
                let extended_span = span.ending_at(now);
                TimelineUpdate {
                    span_writes: vec![SpanWrite::RecordWindowFocusSpan(extended_span.clone())],
                    timeline: Self {
                        ongoing_window_focus: OngoingWindowFocus::FocusedWindowSpan(extended_span),
                        ..self
                    },
                }
            }
            OngoingWindowFocus::NoWindowFocusedSince(_) => TimelineUpdate::without_writes(self),
        }
    }

    pub(super) fn following_window_focus_change(
        self,
        change: WindowFocusChange,
        now: DateTime<Utc>,
    ) -> TimelineUpdate<Self> {
        self.ongoing_window_focus_extended_to(now)
            .then(|timeline| match change {
                WindowFocusChange::WindowGainedFocus(focused) => {
                    timeline.focusing_window(&focused, now)
                }
                WindowFocusChange::NoWindowFocused => timeline.losing_window_focus(now),
            })
    }

    fn focusing_window(self, focused: &Focused, now: DateTime<Utc>) -> TimelineUpdate<Self> {
        let next_span = FocusSpan {
            window: focused.window,
            units: focused.units(),
            started_at: now,
            ended_at: now,
        };
        let continues_ongoing_span = self
            .ongoing_window_focus
            .focused_window_span()
            .is_some_and(|ongoing| ongoing.continues(&next_span));
        if continues_ongoing_span {
            return TimelineUpdate::without_writes(self);
        }
        TimelineUpdate {
            span_writes: vec![SpanWrite::RecordWindowFocusSpan(next_span.clone())],
            timeline: self
                .replacing_ongoing_window_focus(OngoingWindowFocus::FocusedWindowSpan(next_span)),
        }
    }

    fn losing_window_focus(self, now: DateTime<Utc>) -> TimelineUpdate<Self> {
        match self.ongoing_window_focus {
            OngoingWindowFocus::FocusedWindowSpan(_) => TimelineUpdate::without_writes(
                self.replacing_ongoing_window_focus(OngoingWindowFocus::NoWindowFocusedSince(now)),
            ),
            OngoingWindowFocus::NoWindowFocusedSince(_) => TimelineUpdate::without_writes(self),
        }
    }

    fn replacing_ongoing_window_focus(self, next: OngoingWindowFocus) -> Self {
        Self {
            finished_window_focus_spans: self
                .finished_window_focus_spans
                .into_iter()
                .chain(self.ongoing_window_focus.focused_window_span().cloned())
                .collect(),
            ongoing_window_focus: next,
        }
    }

    pub(super) fn focused_window_span(&self) -> Option<&FocusSpan> {
        self.ongoing_window_focus.focused_window_span()
    }

    pub(super) fn focused_units(&self) -> &[FocusUnit] {
        self.focused_window_span()
            .map_or(&[], |span| span.units.as_slice())
    }

    pub(super) fn finished_window_focus_spans(&self) -> &[FocusSpan] {
        &self.finished_window_focus_spans
    }

    pub(super) fn pieces_within(
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
