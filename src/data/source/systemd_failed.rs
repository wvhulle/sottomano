use std::{
    cmp::Reverse,
    marker::PhantomData,
    time::{Duration, SystemTime},
};

use crate::data::source::{
    BarWidget, Glyph, Level, LogEntry, TooltipBody, font_awesome::Solid, journal::FailedUnitInfo,
};

const HORIZON: Duration = Duration::from_secs(3600);

pub trait JournalScope {
    const PER_USER: bool;
    const CLI_FLAG: Option<&'static str>;
    const LOGGED_BY: &'static str;
    const LOGGED_ABOUT: &'static str;
    const ICON: Solid;

    fn severity(recent: bool) -> Level;
}

#[derive(Debug, Clone)]
pub struct User;

impl JournalScope for User {
    const PER_USER: bool = true;
    const CLI_FLAG: Option<&'static str> = Some("--user");
    const LOGGED_BY: &'static str = "_SYSTEMD_USER_UNIT";
    const LOGGED_ABOUT: &'static str = "USER_UNIT";
    const ICON: Solid = Solid::User;

    fn severity(_recent: bool) -> Level {
        Level::Warning
    }
}

#[derive(Debug, Clone)]
pub struct System;

impl JournalScope for System {
    const PER_USER: bool = false;
    const CLI_FLAG: Option<&'static str> = None;
    const LOGGED_BY: &'static str = "_SYSTEMD_UNIT";
    const LOGGED_ABOUT: &'static str = "UNIT";
    const ICON: Solid = Solid::Server;

    fn severity(recent: bool) -> Level {
        if recent {
            Level::Critical
        } else {
            Level::Warning
        }
    }
}

#[derive(Debug, Clone)]
pub struct Failed<S: JournalScope> {
    units: Vec<FailedUnitInfo>,
    scope: PhantomData<S>,
}

impl<S: JournalScope> Failed<S> {
    pub fn from_units(units: Vec<FailedUnitInfo>) -> Option<Self> {
        (!units.is_empty()).then_some(Self {
            units,
            scope: PhantomData,
        })
    }

    fn recent(unit: &FailedUnitInfo) -> bool {
        unit.at
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age <= HORIZON)
    }

    pub fn follow_log_args(&self) -> Vec<String> {
        let scope = S::CLI_FLAG.map(String::from);
        if self.units.is_empty() {
            return scope
                .into_iter()
                .chain(["-x", "-b", "-f", "-p", "warning"].map(String::from))
                .collect();
        }
        scope
            .into_iter()
            .chain(["-x", "-b", "-n", "all", "-f"].map(String::from))
            .chain(self.journal_matches())
            .collect()
    }

    fn journal_matches(&self) -> impl Iterator<Item = String> {
        self.units
            .iter()
            .flat_map(|unit| Self::matches_for(&unit.name))
            .enumerate()
            .flat_map(|(index, matcher)| {
                (index > 0)
                    .then(|| String::from("+"))
                    .into_iter()
                    .chain([matcher])
            })
    }

    fn matches_for(name: &str) -> [String; 4] {
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        [
            format!("{}={name}", S::LOGGED_BY),
            format!("{}={name}", S::LOGGED_ABOUT),
            format!("SYSLOG_IDENTIFIER={stem}"),
            format!("SYSLOG_IDENTIFIER={stem}-start"),
        ]
    }
}

impl<S: JournalScope> BarWidget for Failed<S> {
    fn icon(&self) -> Glyph {
        S::ICON.into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.units.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        S::severity(self.units.iter().any(Self::recent))
    }

    fn tooltip(&self) -> TooltipBody {
        let mut units: Vec<&FailedUnitInfo> = self.units.iter().collect();
        units.sort_by_key(|unit| Reverse(unit.at));
        TooltipBody::Log(
            units
                .into_iter()
                .map(|unit| LogEntry {
                    at: unit.at.unwrap_or_else(SystemTime::now),
                    message: format!(
                        "{} ({})",
                        unit.name,
                        describe_failure(&unit.result, unit.status)
                    ),
                    level: S::severity(Self::recent(unit)),
                })
                .collect(),
        )
    }
}

fn describe_failure(result: &str, status: i32) -> String {
    match result {
        "exit-code" => format!("exited with code {status}"),
        "signal" => format!("killed by signal {status}"),
        "core-dump" => "crashed (core dumped)".to_string(),
        "timeout" => "timed out".to_string(),
        "watchdog" => "watchdog timeout".to_string(),
        "oom-kill" => "killed (out of memory)".to_string(),
        "start-limit-hit" => "too many restarts".to_string(),
        "resources" => "could not start (resources)".to_string(),
        other => other.to_string(),
    }
}
