use std::{borrow::Cow, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};

use super::{
    DurationTimeline, MarkShape, NowCursor, RailPresence, SpanInk, SpanWeight, TimelineAxis,
    TimelineDirection, TimelineMark, TimelineScale, TimelineSpan,
};
use crate::theming::TextFace;

const APPROXIMATE_TIME_PREFIX: &str = "~";
const CAPTION_SEPARATOR: &str = " · ";

pub trait TimelineTextFormat {
    fn clock_time_text(&self, at: DateTime<Utc>) -> String;
    fn length_text(&self, length: TimeDelta) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventGlyph {
    Svg(&'static [u8]),
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePrecision {
    Exact,
    Approximate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventMeasure {
    Unmeasured,
    OffsetFromAnchor(TimeDelta),
    Lasting(TimeDelta),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEvent {
    pub at: DateTime<Utc>,
    pub precision: TimePrecision,
    pub glyph: EventGlyph,
    pub title: Cow<'static, str>,
    pub measure: EventMeasure,
    pub ink: SpanInk,
    pub rail_presence: RailPresence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeEvent {
    pub glyph: EventGlyph,
    pub title: Cow<'static, str>,
    pub ink: SpanInk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanEdges {
    Marked { start: EdgeEvent, end: EdgeEvent },
    Unmarked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanCaption {
    TitledLength(Cow<'static, str>),
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClockSpan {
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
    pub ink: SpanInk,
    pub weight: SpanWeight,
    pub edges: SpanEdges,
    pub caption: SpanCaption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockScale {
    Linear,
    Logarithmic { knee: TimeDelta },
}

#[derive(Debug, Clone, PartialEq)]
pub struct WallClockTimeline {
    now: DateTime<Utc>,
    lookback: TimeDelta,
    minimum_span: TimeDelta,
    end_margin_fraction: f64,
    scale: ClockScale,
    direction: TimelineDirection,
    events: Vec<TimelineEvent>,
    clock_spans: Vec<ClockSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl ClockWindow {
    #[must_use]
    pub fn offset_of(self, at: DateTime<Utc>) -> Duration {
        (at.clamp(self.start, self.end) - self.start)
            .to_std()
            .expect("a moment clamped into the window lies after its start")
    }
}

fn glyph_svgs(glyph: EventGlyph) -> Vec<&'static [u8]> {
    match glyph {
        EventGlyph::Svg(svg) => vec![svg],
        EventGlyph::Plain => Vec::new(),
    }
}

fn event_caption(
    title: &str,
    measure: EventMeasure,
    text_format: &impl TimelineTextFormat,
) -> String {
    match measure {
        EventMeasure::Unmeasured => title.to_owned(),
        EventMeasure::OffsetFromAnchor(offset) => {
            format!(
                "{title}{CAPTION_SEPARATOR}+{}",
                text_format.length_text(offset)
            )
        }
        EventMeasure::Lasting(length) => {
            format!(
                "{title}{CAPTION_SEPARATOR}{}",
                text_format.length_text(length)
            )
        }
    }
}

impl WallClockTimeline {
    #[must_use]
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now,
            lookback: TimeDelta::zero(),
            minimum_span: TimeDelta::zero(),
            end_margin_fraction: 0.0,
            scale: ClockScale::Linear,
            direction: TimelineDirection::OldestFirst,
            events: Vec::new(),
            clock_spans: Vec::new(),
        }
    }

    #[must_use]
    pub fn lookback(self, lookback: TimeDelta) -> Self {
        Self { lookback, ..self }
    }

    #[must_use]
    pub fn minimum_span(self, minimum_span: TimeDelta) -> Self {
        Self {
            minimum_span,
            ..self
        }
    }

    #[must_use]
    pub fn end_margin_fraction(self, end_margin_fraction: f64) -> Self {
        Self {
            end_margin_fraction,
            ..self
        }
    }

    #[must_use]
    pub fn scale(self, scale: ClockScale) -> Self {
        Self { scale, ..self }
    }

    #[must_use]
    pub fn direction(self, direction: TimelineDirection) -> Self {
        Self { direction, ..self }
    }

    #[must_use]
    pub fn events(mut self, events: impl IntoIterator<Item = TimelineEvent>) -> Self {
        self.events.extend(events);
        self
    }

    #[must_use]
    pub fn clock_spans(mut self, clock_spans: impl IntoIterator<Item = ClockSpan>) -> Self {
        self.clock_spans.extend(clock_spans);
        self
    }

    fn event_times(&self) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.events
            .iter()
            .map(|event| event.at)
            .chain(
                self.clock_spans
                    .iter()
                    .flat_map(|clock_span| [clock_span.from, clock_span.until]),
            )
            .chain([self.now])
    }

    #[must_use]
    pub fn window(&self) -> ClockWindow {
        let earliest_drawn = self.now - self.lookback;
        let start = self
            .event_times()
            .min()
            .map_or(self.now, |earliest| earliest.max(earliest_drawn));
        let latest = self.event_times().max().unwrap_or(self.now);
        let span = (latest - start).max(self.minimum_span);
        let margin_seconds = span.as_seconds_f64() * self.end_margin_fraction;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a margin of a timeline spanning days fits i64 milliseconds"
        )]
        let margin = TimeDelta::milliseconds((margin_seconds * 1000.0) as i64);
        ClockWindow {
            start,
            end: start + span + margin,
        }
    }

    fn visible_clock_spans(&self, window: ClockWindow) -> impl Iterator<Item = &ClockSpan> {
        self.clock_spans
            .iter()
            .filter(move |clock_span| clock_span.until > window.start)
    }

    #[must_use]
    pub fn spans(&self) -> Vec<TimelineSpan> {
        let window = self.window();
        self.visible_clock_spans(window)
            .map(|clock_span| TimelineSpan {
                from: window.offset_of(clock_span.from),
                until: window.offset_of(clock_span.until),
                ink: clock_span.ink,
                weight: clock_span.weight,
            })
            .collect()
    }

    fn marks(
        &self,
        window: ClockWindow,
        text_format: &impl TimelineTextFormat,
    ) -> Vec<TimelineMark> {
        let point_mark = |at: DateTime<Utc>,
                          time_text: String,
                          glyph: EventGlyph,
                          detail_text: String,
                          ink: SpanInk,
                          rail_presence: RailPresence| TimelineMark {
            at: window.offset_of(at),
            shape: MarkShape::PointEvent,
            glyph_svgs: glyph_svgs(glyph),
            time_text,
            detail_text,
            ink,
            rail_presence,
        };
        let edge_mark = |at: DateTime<Utc>, edge: &EdgeEvent| {
            point_mark(
                at,
                text_format.clock_time_text(at),
                edge.glyph,
                edge.title.clone().into_owned(),
                edge.ink,
                RailPresence::LabeledColumnOnly,
            )
        };
        let event_marks = self
            .events
            .iter()
            .filter(|event| event.at >= window.start)
            .map(|event| {
                let clock_text = text_format.clock_time_text(event.at);
                point_mark(
                    event.at,
                    match event.precision {
                        TimePrecision::Exact => clock_text,
                        TimePrecision::Approximate => {
                            format!("{APPROXIMATE_TIME_PREFIX}{clock_text}")
                        }
                    },
                    event.glyph,
                    event_caption(&event.title, event.measure, text_format),
                    event.ink,
                    event.rail_presence,
                )
            });
        let edge_marks = self
            .visible_clock_spans(window)
            .flat_map(|clock_span| match &clock_span.edges {
                SpanEdges::Marked { start, end } => {
                    let start_mark = (clock_span.from >= window.start)
                        .then(|| edge_mark(clock_span.from, start));
                    let end_mark =
                        (clock_span.until < self.now).then(|| edge_mark(clock_span.until, end));
                    start_mark.into_iter().chain(end_mark).collect::<Vec<_>>()
                }
                SpanEdges::Unmarked => Vec::new(),
            });
        let length_marks =
            self.visible_clock_spans(window)
                .filter_map(|clock_span| match &clock_span.caption {
                    SpanCaption::TitledLength(title) => Some(TimelineMark {
                        at: window.offset_of(clock_span.from),
                        shape: MarkShape::Interval {
                            until: window.offset_of(clock_span.until),
                        },
                        glyph_svgs: Vec::new(),
                        time_text: String::new(),
                        detail_text: format!(
                            "{title} {}",
                            text_format.length_text(clock_span.until - clock_span.from)
                        ),
                        ink: clock_span.ink,
                        rail_presence: RailPresence::LabeledColumnOnly,
                    }),
                    SpanCaption::Hidden => None,
                });
        event_marks.chain(edge_marks).chain(length_marks).collect()
    }

    #[must_use]
    pub fn duration_timeline(
        &self,
        label_face: TextFace,
        text_format: &impl TimelineTextFormat,
    ) -> DurationTimeline {
        let window = self.window();
        let scale = match self.scale {
            ClockScale::Linear => TimelineScale::Linear,
            ClockScale::Logarithmic { knee } => TimelineScale::Logarithmic {
                knee: knee.to_std().unwrap_or(Duration::ZERO),
            },
        };
        DurationTimeline::new(
            TimelineAxis {
                span_end: window.offset_of(window.end),
                scale,
            },
            label_face,
        )
        .spans(self.spans())
        .marks(self.marks(window, text_format))
        .now_cursor(NowCursor::Shown(window.offset_of(self.now)))
        .direction(self.direction)
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, time::Duration};

    use chrono::{DateTime, TimeDelta, Utc};

    use super::{
        ClockSpan, EdgeEvent, EventGlyph, EventMeasure, SpanCaption, SpanEdges, TimePrecision,
        TimelineEvent, TimelineTextFormat, WallClockTimeline,
    };
    use crate::gauges::{RailPresence, SpanInk, SpanWeight, TimelineSpan};

    struct MinuteText;

    impl TimelineTextFormat for MinuteText {
        fn clock_time_text(&self, at: DateTime<Utc>) -> String {
            at.format("%H:%M").to_string()
        }

        fn length_text(&self, length: TimeDelta) -> String {
            format!("{}m", length.num_minutes())
        }
    }

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn edge(title: &'static str) -> EdgeEvent {
        EdgeEvent {
            glyph: EventGlyph::Plain,
            title: Cow::Borrowed(title),
            ink: SpanInk::Strong,
        }
    }

    fn due_event(minutes: i64, measure: EventMeasure) -> TimelineEvent {
        TimelineEvent {
            at: at(minutes),
            precision: TimePrecision::Approximate,
            glyph: EventGlyph::Plain,
            title: Cow::Borrowed("Due"),
            measure,
            ink: SpanInk::Strong,
            rail_presence: RailPresence::LabeledColumnOnly,
        }
    }

    fn break_span(from: i64, until: i64) -> ClockSpan {
        ClockSpan {
            from: at(from),
            until: at(until),
            ink: SpanInk::Strong,
            weight: SpanWeight::Thin,
            edges: SpanEdges::Marked {
                start: edge("Idle"),
                end: edge("Active"),
            },
            caption: SpanCaption::TitledLength(Cow::Borrowed("Break")),
        }
    }

    fn mark_texts(timeline: &WallClockTimeline) -> Vec<(String, String)> {
        timeline
            .marks(timeline.window(), &MinuteText)
            .into_iter()
            .map(|mark| (mark.time_text, mark.detail_text))
            .collect()
    }

    #[test]
    fn the_window_starts_at_the_earliest_event_inside_the_lookback() {
        let timeline = WallClockTimeline::new(at(100))
            .lookback(TimeDelta::minutes(60))
            .clock_spans([break_span(20, 30), break_span(70, 80)]);
        assert_eq!(timeline.window().start, at(40));
        let timeline = WallClockTimeline::new(at(100))
            .lookback(TimeDelta::hours(24))
            .clock_spans([break_span(20, 30)]);
        assert_eq!(timeline.window().start, at(20));
    }

    #[test]
    fn the_window_ends_past_the_latest_event_by_its_margin() {
        let timeline = WallClockTimeline::new(at(0))
            .end_margin_fraction(0.5)
            .events([due_event(60, EventMeasure::Unmeasured)]);
        assert_eq!(timeline.window().end, at(90));
    }

    #[test]
    fn spans_before_the_window_are_dropped_and_overlapping_ones_clamped() {
        let timeline = WallClockTimeline::new(at(100))
            .lookback(TimeDelta::minutes(60))
            .clock_spans([break_span(10, 20), break_span(30, 50)]);
        assert_eq!(
            timeline.spans(),
            vec![TimelineSpan {
                from: Duration::ZERO,
                until: Duration::from_mins(10),
                ink: SpanInk::Strong,
                weight: SpanWeight::Thin,
            }]
        );
    }

    #[test]
    fn a_finished_span_has_an_event_at_each_end_and_its_length_beside_the_rail() {
        let timeline = WallClockTimeline::new(at(100))
            .lookback(TimeDelta::hours(1))
            .clock_spans([break_span(50, 60)])
            .events([due_event(
                120,
                EventMeasure::OffsetFromAnchor(TimeDelta::minutes(45)),
            )]);
        assert_eq!(
            mark_texts(&timeline),
            vec![
                ("~12:00".to_owned(), "Due · +45m".to_owned()),
                ("10:50".to_owned(), "Idle".to_owned()),
                ("11:00".to_owned(), "Active".to_owned()),
                (String::new(), "Break 10m".to_owned()),
            ]
        );
    }

    #[test]
    fn an_ongoing_span_leaves_its_end_to_the_now_event() {
        let timeline = WallClockTimeline::new(at(100))
            .lookback(TimeDelta::hours(1))
            .clock_spans([break_span(90, 100)]);
        let details: Vec<String> = mark_texts(&timeline)
            .into_iter()
            .map(|(_, detail_text)| detail_text)
            .collect();
        assert_eq!(details, ["Idle", "Break 10m"]);
    }
}
