use std::{borrow::Cow, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};

use super::{
    ClockSpan, EdgeEvent, EventGlyph, EventMeasure, SpanCaption, SpanEdges, TimePrecision,
    TimelineEvent, TimelineTextFormat, WallClockTimeline,
};
use crate::gauges::{MarkMeasure, RailPresence, SpanInk, SpanWeight, TimelineSpan};

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

fn mark_texts(timeline: &WallClockTimeline) -> Vec<(String, String, MarkMeasure)> {
    timeline
        .marks(timeline.window(), &MinuteText)
        .into_iter()
        .map(|mark| (mark.time_text, mark.detail_text, mark.measure))
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
            (
                "~12:00".to_owned(),
                "Due".to_owned(),
                MarkMeasure::Measured("+45m".to_owned())
            ),
            (
                "10:50".to_owned(),
                "Idle".to_owned(),
                MarkMeasure::Unmeasured
            ),
            (
                "11:00".to_owned(),
                "Active".to_owned(),
                MarkMeasure::Unmeasured
            ),
            (
                String::new(),
                "Break 10m".to_owned(),
                MarkMeasure::Unmeasured
            ),
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
        .map(|(_, detail_text, _)| detail_text)
        .collect();
    assert_eq!(details, ["Idle", "Break 10m"]);
}
