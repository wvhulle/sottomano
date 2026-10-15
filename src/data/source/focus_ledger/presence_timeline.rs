use chrono::{DateTime, Utc};

use super::{
    away_reasons::{AwayReasonEdge, LatestAwayReasonEdges},
    focus_units::Presence,
    recorded_spans::{PresenceSpan, SpanWrite, TimelineUpdate},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceTimeline {
    finished_presence_spans: Vec<PresenceSpan>,
    ongoing_presence_span: PresenceSpan,
    latest_away_reason_edges: LatestAwayReasonEdges,
}

impl PresenceTimeline {
    pub(super) fn active_since(started_at: DateTime<Utc>) -> Self {
        Self {
            finished_presence_spans: Vec::new(),
            ongoing_presence_span: PresenceSpan {
                state: Presence::Active,
                started_at,
                ended_at: started_at,
            },
            latest_away_reason_edges: LatestAwayReasonEdges::default(),
        }
    }

    #[must_use]
    pub(super) fn with_loaded_history(self, loaded_presence_spans: Vec<PresenceSpan>) -> Self {
        Self {
            finished_presence_spans: loaded_presence_spans
                .into_iter()
                .filter(|loaded| loaded.started_at < self.ongoing_presence_span.started_at)
                .chain(self.finished_presence_spans)
                .collect(),
            ..self
        }
    }

    pub(super) fn ongoing_presence_extended_to(self, now: DateTime<Utc>) -> TimelineUpdate<Self> {
        let ongoing_presence_span = self.ongoing_presence_span.ending_at(now);
        TimelineUpdate {
            span_writes: vec![SpanWrite::RecordPresenceSpan(ongoing_presence_span.clone())],
            timeline: Self {
                ongoing_presence_span,
                ..self
            },
        }
    }

    pub(super) fn reacting_to_away_reason_edge(
        self,
        edge: AwayReasonEdge,
        now: DateTime<Utc>,
    ) -> TimelineUpdate<Self> {
        let latest_away_reason_edges = self
            .latest_away_reason_edges
            .with_latest_edge_of_its_source(edge);
        let extended = self.ongoing_presence_extended_to(now);
        let presence = latest_away_reason_edges.most_away_presence();
        if presence == extended.timeline.ongoing_presence_span.state {
            return TimelineUpdate {
                timeline: Self {
                    latest_away_reason_edges,
                    ..extended.timeline
                },
                span_writes: extended.span_writes,
            };
        }
        let next_presence_span = PresenceSpan {
            state: presence,
            started_at: now,
            ended_at: now,
        };
        TimelineUpdate {
            span_writes: extended
                .span_writes
                .into_iter()
                .chain([SpanWrite::RecordPresenceSpan(next_presence_span.clone())])
                .collect(),
            timeline: Self {
                finished_presence_spans: extended
                    .timeline
                    .finished_presence_spans
                    .into_iter()
                    .chain([extended.timeline.ongoing_presence_span])
                    .collect(),
                ongoing_presence_span: next_presence_span,
                latest_away_reason_edges,
            },
        }
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

    pub(super) fn first_presence_started_at(&self) -> DateTime<Utc> {
        self.finished_presence_spans
            .first()
            .unwrap_or(&self.ongoing_presence_span)
            .started_at
    }

    pub(super) fn screen_off_since(&self) -> Option<DateTime<Utc>> {
        (self.ongoing_presence_span.state == Presence::ScreenOff)
            .then_some(self.ongoing_presence_span.started_at)
    }
}
