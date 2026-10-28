use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use sottomano_config::widget_groups::focus_time::FocusTimeConfig;

use super::{
    FocusLedger, FocusSignal, away_reasons::ScreenEdge, focus_identity::Focused,
    time_intervals::Interval,
};
use crate::login_session_presence::{
    logind_manager::SleepEdge,
    wayland_idle_notify::{IdleEdge, InputIdleEdge},
};

pub(super) fn at(minutes: i64) -> DateTime<Utc> {
    "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
}

pub(super) fn config() -> FocusTimeConfig {
    FocusTimeConfig {
        idle_after: Duration::from_mins(3),
        ..FocusTimeConfig::default()
    }
}

pub(super) fn focused(app_id: &str, project: Option<&str>) -> FocusSignal {
    focused_in(1, app_id, project)
}

pub(super) fn focused_in(window: u64, app_id: &str, project: Option<&str>) -> FocusSignal {
    FocusSignal::WindowFocused(Focused {
        window,
        app_id: app_id.to_owned(),
        project: project.map(PathBuf::from),
    })
}

pub(super) fn no_window_focused() -> FocusSignal {
    FocusSignal::NoWindowFocused
}

pub(super) fn idle_edge(edge: IdleEdge) -> FocusSignal {
    FocusSignal::Idle(InputIdleEdge::simulated(edge))
}

pub(super) fn sleep_edge(edge: SleepEdge) -> FocusSignal {
    FocusSignal::Sleep(edge)
}

pub(super) fn screen_power_edge(edge: ScreenEdge) -> FocusSignal {
    FocusSignal::ScreenPower(edge)
}

pub(super) struct Step {
    signal: FocusSignal,
    minute: i64,
}

pub(super) fn step(signal: FocusSignal, minute: i64) -> Step {
    Step { signal, minute }
}

pub(super) fn folded(steps: impl IntoIterator<Item = Step>) -> FocusLedger {
    let mut ledger = FocusLedger::started_at(at(0));
    for step in steps {
        ledger.fold(step.signal, at(step.minute), &config());
    }
    ledger
}

pub(super) fn whole_day() -> Interval {
    Interval {
        from: at(-60),
        until: at(600),
    }
}
