use std::{cmp::Reverse, time::SystemTime};

use crate::{
    data::source::{
        journal::FailedUnitInfo,
        systemd_failed::{Failed, JournalScope, System, User},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Level},
        font_awesome::Solid,
        tooltip_body::{LogEntry, TooltipBody},
    },
};

pub trait ScopeAppearance: JournalScope {
    const ICON: Solid;

    fn severity(recent: bool) -> Level;
}

impl ScopeAppearance for User {
    const ICON: Solid = Solid::User;

    fn severity(_recent: bool) -> Level {
        Level::Warning
    }
}

impl ScopeAppearance for System {
    const ICON: Solid = Solid::Server;

    fn severity(recent: bool) -> Level {
        if recent {
            Level::Critical
        } else {
            Level::Warning
        }
    }
}

impl<S: ScopeAppearance> BarWidget for Failed<S> {
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
                    level: S::severity(Failed::<S>::recent(unit)),
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
