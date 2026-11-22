use std::time::Duration;

use chrono::{DateTime, Locale, TimeDelta, Utc};
use corredo::gauges::{SpanInk, SpanWeight, TimelineSpan};
use sottomano_desktop_sources::login_session_presence::{
    focus_time_ledger::{FocusLedger, FocusProject, FocusSignal, Focused, Interval, ScreenEdge},
    logind_manager::SleepEdge,
};
use sottomano_toml_config::indicator_groups::session_focus::SessionFocusConfig;

use super::{DueMoment, FocusTimeline, RunOffset, SessionDue};
use crate::indicator_groups::session_focus_group::{WarningSnooze, session_face::SessionFace};

fn at(minutes: i64) -> DateTime<Utc> {
    "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
}

fn focused(window: u64) -> FocusSignal {
    FocusSignal::WindowFocused(Focused {
        window,
        app_id: "kitty".to_owned(),
        project: FocusProject::OutsideRepository,
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
    Duration::from_mins(minutes - 20)
}

#[test]
fn breaks_are_drawn_by_cause_and_the_current_run_stands_out() {
    let config = config();
    let ledger = ledger_with_two_breaks();
    let timeline = FocusTimeline {
        session_face: SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::NotSnoozed,
        },
    };
    assert_eq!(
        timeline.clock_timeline(at(130)).spans(),
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
            snooze: WarningSnooze::NotSnoozed,
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
    assert!(timeline.clock_timeline(at(130)).window().end > at(190));
}

#[test]
fn during_a_break_there_is_no_run_and_no_due_moment() {
    let config = config();
    let ledger = ledger_with_two_breaks();
    let timeline = FocusTimeline {
        session_face: SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::NotSnoozed,
        },
    };
    assert_eq!(timeline.due_moments(at(80)), Vec::new());
    assert!(
        timeline
            .clock_timeline(at(80))
            .spans()
            .iter()
            .all(|span| (span.ink, span.weight) != (SpanInk::Accent, SpanWeight::Thick))
    );
}

#[test]
fn the_row_says_when_a_break_is_due_or_how_long_the_break_lasts() {
    let config = config();
    let ledger = ledger_with_two_breaks();
    let timeline = FocusTimeline {
        session_face: SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::NotSnoozed,
        },
    };
    assert_eq!(
        timeline.break_due_text(at(130), Locale::en_US),
        "Break due in 15m"
    );
    assert_eq!(
        timeline.break_due_text(at(160), Locale::en_US),
        "Break overdue 15m"
    );
    assert_eq!(
        timeline.break_due_text(at(80), Locale::en_US),
        "On break 20m"
    );
}

#[test]
fn the_longest_past_session_gets_its_own_span() {
    let config = config();
    let ledger = ledger_with_two_breaks();
    let timeline = FocusTimeline {
        session_face: SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::NotSnoozed,
        },
    };
    assert_eq!(
        timeline.longest_past_session(at(80)),
        Some(Interval {
            from: at(35),
            until: at(60),
        })
    );
}

#[test]
fn the_now_mark_names_the_unit_of_the_current_run() {
    let config = config();
    let ledger = ledger_with_two_breaks();
    let timeline = FocusTimeline {
        session_face: SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::NotSnoozed,
        },
    };
    assert_eq!(timeline.now_event(at(130)).title, "Now on kitty");
    assert_eq!(timeline.now_event(at(80)).title, "Now on a break");
}
