use std::{path::PathBuf, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};

use super::{
    FocusLedger, FocusSignal, focus_identity::Focused, focus_units::FocusUnit,
    recorded_spans::FocusScope, time_intervals::Interval,
};
use crate::config::widget_groups::focus_time::FocusTimeConfig;

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
    FocusSignal::Focused(Some(Focused {
        window,
        app_id: app_id.to_owned(),
        project: project.map(PathBuf::from),
    }))
}

pub(super) fn kitty() -> FocusScope {
    FocusScope::Unit(FocusUnit::App("kitty".to_owned()))
}

pub(super) struct Step {
    signal: FocusSignal,
    minute: i64,
}

pub(super) fn step(signal: FocusSignal, minute: i64) -> Step {
    Step { signal, minute }
}

pub(super) fn folded(steps: impl IntoIterator<Item = Step>) -> FocusLedger {
    steps
        .into_iter()
        .fold(FocusLedger::default(), |ledger, step| {
            ledger.fold(step.signal, at(step.minute)).ledger
        })
}

pub(super) fn whole_day() -> Interval {
    Interval {
        from: at(-60),
        until: at(600),
    }
}
