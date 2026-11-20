use std::time::Duration;

use chrono::{DateTime, Locale, TimeDelta, Utc};
use corredo::{
    gauges::{
        DurationTimeline, NowCursor, RailPresence, SpanInk, SpanWeight, TimelineAxis, TimelineMark,
        TimelineScale, TimelineSpan,
    },
    theming::TextFace,
};
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, BreakCause, Interval, LAST_DAY, ScreenRun, TakenBreak,
};

use super::session_face::SessionFace;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    widgets::{localized_durations::duration_text, localized_time::LocalizedTimeFormat},
};

const FOCUS_TIMELINE_LOOKBACK: TimeDelta = TimeDelta::hours(4);
const SPAN_END_MARGIN_DIVISOR: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TimelineWindow {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl TimelineWindow {
    fn offset_of(self, at: DateTime<Utc>) -> Duration {
        (at.clamp(self.start, self.end) - self.start)
            .to_std()
            .expect("a moment clamped into the window lies after its start")
    }
}

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

fn break_span(window: TimelineWindow, taken: &TakenBreak) -> TimelineSpan {
    let (ink, weight) = match taken.cause {
        BreakCause::Asleep => (SpanInk::Strong, SpanWeight::Thick),
        BreakCause::ScreenOff | BreakCause::LongIdle => (SpanInk::Strong, SpanWeight::Thin),
    };
    TimelineSpan {
        from: window.offset_of(taken.interval.from),
        until: window.offset_of(taken.interval.until),
        ink,
        weight,
    }
}

pub(super) struct FocusTimeline<'a> {
    pub session_face: SessionFace<'a>,
}

impl<'a> FocusTimeline<'a> {
    fn accounting(&self) -> Accounting<'a> {
        self.session_face.accounting
    }

    pub(super) fn window(&self, now: DateTime<Utc>) -> TimelineWindow {
        let config = self.accounting().config;
        let horizon = match self.accounting().screen_run(now) {
            Some(ScreenRun { since }) => (since + config.session_critical).max(now),
            None => now + config.session_warning,
        };
        let start = now - FOCUS_TIMELINE_LOOKBACK;
        TimelineWindow {
            start,
            end: horizon + (horizon - start) / SPAN_END_MARGIN_DIVISOR,
        }
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

    pub(super) fn breaks_in_window(&self, now: DateTime<Utc>) -> Vec<TakenBreak> {
        let window_start = self.window(now).start;
        self.accounting()
            .taken_breaks_within(LAST_DAY.ending_at(now), now)
            .into_iter()
            .filter(|taken| taken.interval.until > window_start)
            .collect()
    }

    pub(super) fn timeline_spans(&self, now: DateTime<Utc>) -> Vec<TimelineSpan> {
        let window = self.window(now);
        let break_spans = self
            .breaks_in_window(now)
            .into_iter()
            .map(|taken| break_span(window, &taken))
            .collect::<Vec<_>>();
        let run_span = self
            .accounting()
            .screen_run(now)
            .map(|ScreenRun { since }| TimelineSpan {
                from: window.offset_of(since),
                until: window.offset_of(now),
                ink: SpanInk::Accent,
                weight: SpanWeight::Thick,
            });
        break_spans.into_iter().chain(run_span).collect()
    }

    pub(super) fn timeline(
        &self,
        now: DateTime<Utc>,
        locale: Locale,
        label_face: TextFace,
    ) -> DurationTimeline {
        let window = self.window(now);
        let clock_text = |at: DateTime<Utc>| LocalizedTimeFormat::HourAndMinute.format(&at, locale);
        let due_marks = self.due_moments(now).into_iter().map(|moment| {
            let detail_text = match moment.after_run_start {
                RunOffset::Counted(offset) => {
                    format!(
                        "{} · +{}",
                        moment.due.title(),
                        duration_text(offset, locale)
                    )
                }
                RunOffset::Uncounted => moment.due.title().to_owned(),
            };
            TimelineMark {
                at: window.offset_of(moment.at),
                glyph_svgs: vec![moment.due.glyph().svg_data()],
                time_text: clock_text(moment.at),
                detail_text,
                ink: moment.due.ink(),
                rail_presence: RailPresence::OnCompactRail,
            }
        });
        let break_marks = self.breaks_in_window(now).into_iter().map(|taken| {
            let Interval { from, until } = taken.interval;
            let detail_text = if until >= now {
                format!("Break · {}, ongoing", taken.cause.label())
            } else {
                format!(
                    "Break {} · {}",
                    duration_text(until - from, locale),
                    taken.cause.label()
                )
            };
            TimelineMark {
                at: window.offset_of(from),
                glyph_svgs: vec![break_glyph(taken.cause).svg_data()],
                time_text: clock_text(from),
                detail_text,
                ink: SpanInk::Strong,
                rail_presence: RailPresence::LabeledColumnOnly,
            }
        });
        let now_detail = match self.accounting().screen_run(now) {
            Some(ScreenRun { since }) => format!(
                "Now · focused {}",
                duration_text(TimeDelta::minutes((now - since).num_minutes()), locale)
            ),
            None => "Now · on a break".to_owned(),
        };
        let now_mark = TimelineMark {
            at: window.offset_of(now),
            glyph_svgs: Vec::new(),
            time_text: clock_text(now),
            detail_text: now_detail,
            ink: SpanInk::Strong,
            rail_presence: RailPresence::LabeledColumnOnly,
        };
        DurationTimeline::new(
            TimelineAxis {
                span_end: window.offset_of(window.end),
                scale: TimelineScale::Linear,
            },
            label_face,
        )
        .spans(self.timeline_spans(now))
        .marks(break_marks.chain([now_mark]).chain(due_marks).collect())
        .now_cursor(NowCursor::Shown(window.offset_of(now)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{DateTime, TimeDelta, Utc};
    use corredo::gauges::{SpanInk, SpanWeight, TimelineSpan};
    use sottomano_desktop_sources::login_session_presence::{
        focus_time_ledger::{FocusLedger, FocusSignal, Focused, ScreenEdge},
        logind_manager::SleepEdge,
    };
    use sottomano_toml_config::indicator_groups::session_focus::SessionFocusConfig;

    use super::{DueMoment, FocusTimeline, RunOffset, SessionDue};
    use crate::indicator_groups::session_focus_group::session_face::SessionFace;

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn focused(window: u64) -> FocusSignal {
        FocusSignal::WindowFocused(Focused {
            window,
            app_id: "kitty".to_owned(),
            project: None,
        })
    }

    fn config() -> SessionFocusConfig {
        SessionFocusConfig {
            idle_after: TimeDelta::minutes(3),
            session_warning: TimeDelta::minutes(45),
            session_critical: TimeDelta::minutes(90),
            ..SessionFocusConfig::default()
        }
    }

    fn ledger_with_two_breaks() -> FocusLedger {
        let config = config();
        let mut ledger = FocusLedger::started_at(at(0));
        for (signal, minute) in [
            (focused(1), 0),
            (FocusSignal::ScreenPower(ScreenEdge::Off), 20),
            (FocusSignal::ScreenPower(ScreenEdge::On), 35),
            (focused(1), 35),
            (FocusSignal::Sleep(SleepEdge::Suspending), 60),
            (FocusSignal::Sleep(SleepEdge::Resumed), 100),
            (focused(2), 100),
        ] {
            ledger.fold(signal, at(minute), &config);
        }
        ledger
    }

    fn minutes_after_window_start(minutes: u64) -> Duration {
        Duration::from_mins(minutes + 110)
    }

    #[test]
    fn breaks_are_drawn_by_cause_and_the_current_run_stands_out() {
        let config = config();
        let ledger = ledger_with_two_breaks();
        let timeline = FocusTimeline {
            session_face: SessionFace {
                accounting: ledger.accounted(&config),
                snooze: None,
            },
        };
        assert_eq!(
            timeline.timeline_spans(at(130)),
            vec![
                TimelineSpan {
                    from: minutes_after_window_start(20),
                    until: minutes_after_window_start(35),
                    ink: SpanInk::Strong,
                    weight: SpanWeight::Thin,
                },
                TimelineSpan {
                    from: minutes_after_window_start(60),
                    until: minutes_after_window_start(100),
                    ink: SpanInk::Strong,
                    weight: SpanWeight::Thick,
                },
                TimelineSpan {
                    from: minutes_after_window_start(100),
                    until: minutes_after_window_start(130),
                    ink: SpanInk::Accent,
                    weight: SpanWeight::Thick,
                },
            ]
        );
    }

    #[test]
    fn warning_and_critical_follow_the_end_of_the_last_break() {
        let config = config();
        let ledger = ledger_with_two_breaks();
        let timeline = FocusTimeline {
            session_face: SessionFace {
                accounting: ledger.accounted(&config),
                snooze: None,
            },
        };
        assert_eq!(
            timeline.due_moments(at(130)),
            vec![
                DueMoment {
                    due: SessionDue::Warning,
                    at: at(145),
                    after_run_start: RunOffset::Counted(TimeDelta::minutes(45)),
                },
                DueMoment {
                    due: SessionDue::Critical,
                    at: at(190),
                    after_run_start: RunOffset::Counted(TimeDelta::minutes(90)),
                },
            ]
        );
        assert!(timeline.window(at(130)).end > at(190));
    }

    #[test]
    fn during_a_break_there_is_no_run_and_no_due_moment() {
        let config = config();
        let ledger = ledger_with_two_breaks();
        let timeline = FocusTimeline {
            session_face: SessionFace {
                accounting: ledger.accounted(&config),
                snooze: None,
            },
        };
        assert_eq!(timeline.due_moments(at(80)), Vec::new());
        assert!(
            timeline
                .timeline_spans(at(80))
                .iter()
                .all(|span| span.ink != SpanInk::Accent)
        );
    }
}
