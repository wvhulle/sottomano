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
    pub(super) fn interval(&self) -> Interval {
        Interval {
            from: self.started_at,
            until: self.ended_at,
        }
    }

    pub(super) fn piece(&self) -> FocusPiece<'_> {
        FocusPiece {
            interval: self.interval(),
            window: self.window,
            units: &self.units,
        }
    }

    pub(super) fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self { ended_at, ..self }
    }

    pub(super) fn continues(&self, other: &FocusSpan) -> bool {
        self.window == other.window && self.units == other.units
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FocusPiece<'a> {
    pub interval: Interval,
    pub window: u64,
    pub units: &'a [FocusUnit],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusScope {
    Unit(FocusUnit),
    Window(u64),
}

impl FocusScope {
    pub(super) fn covers(&self, piece: &FocusPiece) -> bool {
        match self {
            FocusScope::Unit(unit) => piece.units.contains(unit),
            FocusScope::Window(window) => piece.window == *window,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineUpdate<Timeline> {
    pub timeline: Timeline,
    pub span_writes: Vec<SpanWrite>,
}

impl<Timeline> TimelineUpdate<Timeline> {
    pub(super) fn without_writes(timeline: Timeline) -> Self {
        Self {
            timeline,
            span_writes: Vec::new(),
        }
    }

    pub(super) fn then(self, step: impl FnOnce(Timeline) -> TimelineUpdate<Timeline>) -> Self {
        let next = step(self.timeline);
        Self {
            timeline: next.timeline,
            span_writes: self
                .span_writes
                .into_iter()
                .chain(next.span_writes)
                .collect(),
        }
    }
}
