use std::{
    cmp::Reverse,
    time::{Duration, SystemTime},
};

use crate::data::source::{
    BarWidget, Glyph, Level, LogEntry, TooltipBody, font_awesome::Solid, journal::FailedUnitInfo,
};

const HORIZON: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
pub struct Failed<const USER: bool> {
    units: Vec<FailedUnitInfo>,
}

impl<const USER: bool> Failed<USER> {
    pub fn from_units(units: Vec<FailedUnitInfo>) -> Option<Self> {
        (!units.is_empty()).then_some(Self { units })
    }

    fn recent(unit: &FailedUnitInfo) -> bool {
        unit.at
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age <= HORIZON)
    }

    fn severity(recent: bool) -> Level {
        if !USER && recent {
            Level::Critical
        } else {
            Level::Warning
        }
    }
}

impl<const USER: bool> BarWidget for Failed<USER> {
    fn icon(&self) -> Glyph {
        if USER {
            Solid::User.into()
        } else {
            Solid::Server.into()
        }
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.units.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        Self::severity(!USER && self.units.iter().any(Self::recent))
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
                    level: Self::severity(Self::recent(unit)),
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
