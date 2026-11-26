#[cfg(test)]
mod focus_timeline_tests;
use std::borrow::Cow;

use chrono::{DateTime, Locale, TimeDelta, Utc};
use corredo::{
    gauges::{
        ClockSpan, DurationTimeline, EdgeEvent, EventGlyph, EventMeasure, RailPresence,
        SpanCaption, SpanEdges, SpanInk, SpanWeight, TimePrecision, TimelineDirection,
        TimelineEvent, WallClockTimeline,
    },
    theming::TextFace,
};
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, BreakCause, FocusRun, FocusUnit, Interval, LAST_DAY, ScreenRun, TakenBreak,
};

use super::session_face::SessionFace;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    widgets::{localized_durations::duration_text, localized_timeline_text::LocalizedTimelineText},
};

const FOCUS_TIMELINE_LOOKBACK: TimeDelta = TimeDelta::hours(24);
const END_MARGIN_FRACTION: f64 = 1.0 / 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionDue {
    Warning,
    Critical,
    SnoozeEnds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DueMoment {
    due: SessionDue,
    at: DateTime<Utc>,
    after_run_start: RunOffset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunOffset {
    Counted(TimeDelta),
    Uncounted,
}

impl SessionDue {
    fn glyph(self) -> Phosphor {
        match self {
            SessionDue::Warning => Phosphor::Warning(Weight::Regular),
            SessionDue::Critical => Phosphor::WarningOctagon(Weight::Regular),
            SessionDue::SnoozeEnds => Phosphor::BellSlash(Weight::Regular),
        }
    }

    fn ink(self) -> SpanInk {
        match self {
            SessionDue::Warning => SpanInk::Warning,
            SessionDue::Critical => SpanInk::Critical,
            SessionDue::SnoozeEnds => SpanInk::Accent,
        }
    }

    fn title(self) -> &'static str {
        match self {
            SessionDue::Warning => "Warning",
            SessionDue::Critical => "Critical",
            SessionDue::SnoozeEnds => "Snooze ends",
        }
    }
}

fn break_glyph(cause: BreakCause) -> Phosphor {
    match cause {
        BreakCause::LongIdle => Phosphor::Coffee(Weight::Regular),
        BreakCause::ScreenOff => Phosphor::Monitor(Weight::Regular),
        BreakCause::Asleep => Phosphor::Moon(Weight::Regular),
    }
}

fn focus_unit_glyph(unit: &FocusUnit) -> Phosphor {
    match unit {
        FocusUnit::Project(_) => Phosphor::FolderSimple(Weight::Regular),
        FocusUnit::App(_) => Phosphor::AppWindow(Weight::Regular),
    }
}

fn edge_event(glyph: Phosphor, title: &'static str) -> EdgeEvent {
    EdgeEvent {
        glyph: EventGlyph::Svg(glyph.svg_data()),
        title: Cow::Borrowed(title),
        ink: SpanInk::Strong,
    }
}

fn break_start_title(cause: BreakCause) -> &'static str {
    match cause {
        BreakCause::LongIdle => "Idle",
        BreakCause::ScreenOff => "Screen off",
        BreakCause::Asleep => "Asleep",
    }
}

fn break_span(taken: &TakenBreak) -> ClockSpan {
    let Interval { from, until } = taken.interval;
    let weight = match taken.cause {
        BreakCause::Asleep => SpanWeight::Thick,
        BreakCause::ScreenOff | BreakCause::LongIdle => SpanWeight::Thin,
    };
    ClockSpan {
        from,
        until,
        ink: SpanInk::Strong,
        weight,
        edges: SpanEdges::Marked {
            start: edge_event(break_glyph(taken.cause), break_start_title(taken.cause)),
            end: edge_event(Phosphor::Keyboard(Weight::Regular), "Active again"),
        },
        caption: SpanCaption::TitledLength(Cow::Borrowed("Break")),
    }
}

pub(super) struct FocusTimeline<'view> {
    pub session_face: SessionFace<'view>,
}

impl<'view> FocusTimeline<'view> {
    fn accounting(&self) -> Accounting<'view> {
        self.session_face.accounting
    }

    pub(super) fn due_moments(&self, now: DateTime<Utc>) -> Vec<DueMoment> {
        let config = self.accounting().config;
        let thresholds = self
            .accounting()
            .screen_run(now)
            .map(|ScreenRun { since }| {
                [
                    DueMoment {
                        due: SessionDue::Warning,
                        at: since + config.session_warning,
                        after_run_start: RunOffset::Counted(config.session_warning),
                    },
                    DueMoment {
                        due: SessionDue::Critical,
                        at: since + config.session_critical,
                        after_run_start: RunOffset::Counted(config.session_critical),
                    },
                ]
            });
        let snooze_end = self.session_face.snoozed_until(now).map(|until| DueMoment {
            due: SessionDue::SnoozeEnds,
            at: until,
            after_run_start: RunOffset::Uncounted,
        });
        thresholds.into_iter().flatten().chain(snooze_end).collect()
    }

    pub(super) fn longest_past_session(&self, now: DateTime<Utc>) -> Option<Interval> {
        let current_since = self
            .accounting()
            .screen_run(now)
            .map(|ScreenRun { since }| since);
        self.accounting()
            .longest_session_within(LAST_DAY.ending_at(now), now)
            .filter(|session| Some(session.from) != current_since)
    }

    pub(super) fn now_event(&self, now: DateTime<Utc>) -> TimelineEvent {
        let (title, measure) = match self.accounting().screen_run(now) {
            None => (Cow::Borrowed("Now on a break"), EventMeasure::Unmeasured),
            Some(ScreenRun { since }) => (
                match self.accounting().dominant_run(now) {
                    Some(run) => Cow::Owned(format!("Now on {}", run.unit.label())),
                    None => Cow::Borrowed("Now focused"),
                },
                EventMeasure::Lasting(now - since),
            ),
        };
        TimelineEvent {
            at: now,
            precision: TimePrecision::Exact,
            glyph: EventGlyph::Plain,
            title,
            measure,
            ink: SpanInk::Strong,
            rail_presence: RailPresence::LabeledColumnOnly,
        }
    }

    pub(super) fn break_due_text(&self, now: DateTime<Utc>, locale: Locale) -> String {
        let minutes_text =
            |length: TimeDelta| duration_text(TimeDelta::minutes(length.num_minutes()), locale);
        let Some(ScreenRun { since }) = self.accounting().screen_run(now) else {
            return match self.accounting().last_break(now) {
                Some(Interval { from, .. }) => format!("On break {}", minutes_text(now - from)),
                None => "On break".to_owned(),
            };
        };
        let break_due_at = since + self.accounting().config.session_warning;
        match self.session_face.snoozed_until(now) {
            Some(until) => format!("Break reminder in {}", minutes_text(until - now)),
            None if break_due_at > now => {
                format!("Break due in {}", minutes_text(break_due_at - now))
            }
            None => format!("Break overdue {}", minutes_text(now - break_due_at)),
        }
    }

    fn due_event(moment: DueMoment) -> TimelineEvent {
        TimelineEvent {
            at: moment.at,
            precision: TimePrecision::Exact,
            glyph: EventGlyph::Svg(moment.due.glyph().svg_data()),
            title: Cow::Borrowed(moment.due.title()),
            measure: match moment.after_run_start {
                RunOffset::Counted(offset) => EventMeasure::OffsetFromAnchor(offset),
                RunOffset::Uncounted => EventMeasure::Unmeasured,
            },
            ink: moment.due.ink(),
            rail_presence: RailPresence::OnCompactRail,
        }
    }

    pub(super) fn clock_timeline(&self, now: DateTime<Utc>) -> WallClockTimeline {
        let break_spans = self
            .accounting()
            .taken_breaks_within(LAST_DAY.ending_at(now), now)
            .iter()
            .map(break_span)
            .collect::<Vec<_>>();
        let longest_session_span = self.longest_past_session(now).map(|session| ClockSpan {
            from: session.from,
            until: session.until,
            ink: SpanInk::Accent,
            weight: SpanWeight::Thin,
            edges: SpanEdges::Unmarked,
            caption: SpanCaption::TitledLength(Cow::Borrowed("Longest session")),
        });
        let run_span = self
            .accounting()
            .screen_run(now)
            .map(|ScreenRun { since }| ClockSpan {
                from: since,
                until: now,
                ink: SpanInk::Accent,
                weight: SpanWeight::Thick,
                edges: SpanEdges::Unmarked,
                caption: SpanCaption::Hidden,
            });
        let run_events = self
            .accounting()
            .runs(now)
            .into_iter()
            .map(|FocusRun { unit, since }| TimelineEvent {
                at: since,
                precision: TimePrecision::Exact,
                glyph: EventGlyph::Svg(focus_unit_glyph(&unit).svg_data()),
                title: Cow::Owned(format!("On {} {}", unit.kind_label(), unit.label())),
                measure: EventMeasure::Unmeasured,
                ink: SpanInk::Accent,
                rail_presence: RailPresence::LabeledColumnOnly,
            });
        let due_events = self.due_moments(now).into_iter().map(Self::due_event);
        WallClockTimeline::new(now)
            .lookback(FOCUS_TIMELINE_LOOKBACK)
            .direction(TimelineDirection::NewestFirst)
            .end_margin_fraction(END_MARGIN_FRACTION)
            .clock_spans(
                break_spans
                    .into_iter()
                    .chain(longest_session_span)
                    .chain(run_span),
            )
            .events(run_events.chain([self.now_event(now)]).chain(due_events))
    }

    pub(super) fn timeline(
        &self,
        now: DateTime<Utc>,
        locale: Locale,
        label_face: TextFace,
    ) -> DurationTimeline {
        self.clock_timeline(now)
            .duration_timeline(label_face, &LocalizedTimelineText { locale })
    }
}
