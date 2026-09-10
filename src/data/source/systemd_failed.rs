use std::{marker::PhantomData, time::Duration};

use crate::data::source::journal::FailedUnitInfo;

pub const RECENT_FAILURE_HORIZON: Duration = Duration::from_secs(3600);

pub trait JournalScope {
    const PER_USER: bool;
    const CLI_FLAG: Option<&'static str>;
    const LOGGED_BY: &'static str;
    const LOGGED_ABOUT: &'static str;
}

#[derive(Debug, Clone)]
pub struct User;

impl JournalScope for User {
    const PER_USER: bool = true;
    const CLI_FLAG: Option<&'static str> = Some("--user");
    const LOGGED_BY: &'static str = "_SYSTEMD_USER_UNIT";
    const LOGGED_ABOUT: &'static str = "USER_UNIT";
}

#[derive(Debug, Clone)]
pub struct System;

impl JournalScope for System {
    const PER_USER: bool = false;
    const CLI_FLAG: Option<&'static str> = None;
    const LOGGED_BY: &'static str = "_SYSTEMD_UNIT";
    const LOGGED_ABOUT: &'static str = "UNIT";
}

#[derive(Debug, Clone)]
pub struct Failed<S: JournalScope> {
    pub units: Vec<FailedUnitInfo>,
    scope: PhantomData<S>,
}

impl<S: JournalScope> Failed<S> {
    pub fn from_units(units: Vec<FailedUnitInfo>) -> Option<Self> {
        (!units.is_empty()).then_some(Self {
            units,
            scope: PhantomData,
        })
    }

    pub fn recent(unit: &FailedUnitInfo) -> bool {
        unit.at
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age <= RECENT_FAILURE_HORIZON)
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
