use chrono::{DateTime, Utc};

use super::{
    focus_units::{FocusUnit, Presence},
    time_intervals::Interval,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSpan {
    pub window: u64,
    pub units: Vec<FocusUnit>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

impl FocusSpan {
    pub(super) fn piece(&self) -> FocusPiece<'_> {
        FocusPiece {
            interval: Interval {
                from: self.started_at,
                until: self.ended_at,
            },
            window: self.window,
            units: &self.units,
        }
    }

    pub(super) fn continuity_with(&self, other: &FocusSpan) -> SpanContinuity {
        if self.window == other.window && self.units == other.units {
            SpanContinuity::Continues
        } else {
            SpanContinuity::Breaks
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FocusPiece<'a> {
    pub interval: Interval,
    pub window: u64,
    pub units: &'a [FocusUnit],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceSpan {
    pub state: Presence,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

impl PresenceSpan {
    pub(super) fn interval(&self) -> Interval {
        Interval {
            from: self.started_at,
            until: self.ended_at,
        }
    }

    pub(super) fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self { ended_at, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanWrite {
    RecordWindowFocusSpan(FocusSpan),
    RecordPresenceSpan(PresenceSpan),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadedSpans {
    pub window_focus_spans: Vec<FocusSpan>,
    pub presence_spans: Vec<PresenceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SpanContinuity {
    Continues,
    Breaks,
}
