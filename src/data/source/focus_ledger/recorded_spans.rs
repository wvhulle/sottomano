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

    pub(super) fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self { ended_at, ..self }
    }

    pub(super) fn continues(&self, other: &FocusSpan) -> bool {
        self.window == other.window && self.units == other.units
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusScope {
    Unit(FocusUnit),
    Window(u64),
}

impl FocusScope {
    pub(super) fn covers(&self, span: &FocusSpan) -> bool {
        match self {
            FocusScope::Unit(unit) => span.units.contains(unit),
            FocusScope::Window(window) => span.window == *window,
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
    OpenFocus(FocusSpan),
    ExtendFocus { id: i64, ended_at: DateTime<Utc> },
    OpenPresence(PresenceSpan),
    ExtendPresence { id: i64, ended_at: DateTime<Utc> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenedSpan {
    pub id: i64,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadedSpans {
    pub focus: Vec<FocusSpan>,
    pub presence: Vec<PresenceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowId {
    Pending,
    Stored(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSpan<S> {
    pub span: S,
    pub row: RowId,
}

impl<S> OpenSpan<S> {
    pub(super) fn pending(span: S) -> Self {
        Self {
            span,
            row: RowId::Pending,
        }
    }

    fn stored_id(&self) -> Option<i64> {
        match self.row {
            RowId::Pending => None,
            RowId::Stored(id) => Some(id),
        }
    }

    pub(super) fn stored_as(self, opened: OpenedSpan, started_at: DateTime<Utc>) -> Self {
        match self.row {
            RowId::Pending if started_at == opened.started_at => Self {
                row: RowId::Stored(opened.id),
                ..self
            },
            _ => self,
        }
    }
}

impl OpenSpan<FocusSpan> {
    pub(super) fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self {
            span: self.span.ending_at(ended_at),
            ..self
        }
    }

    pub(super) fn extension(&self) -> Option<SpanWrite> {
        self.stored_id().map(|id| SpanWrite::ExtendFocus {
            id,
            ended_at: self.span.ended_at,
        })
    }
}

impl OpenSpan<PresenceSpan> {
    pub(super) fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self {
            span: self.span.ending_at(ended_at),
            ..self
        }
    }

    pub(super) fn extension(&self) -> Option<SpanWrite> {
        self.stored_id().map(|id| SpanWrite::ExtendPresence {
            id,
            ended_at: self.span.ended_at,
        })
    }
}
