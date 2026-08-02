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

    pub fn follow_log_args(&self) -> Vec<String> {
        let scope = USER.then(|| String::from("--user"));
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
        let logged_by_unit = if USER {
            "_SYSTEMD_USER_UNIT"
        } else {
            "_SYSTEMD_UNIT"
        };
        let logged_about_unit = if USER { "USER_UNIT" } else { "UNIT" };
        [
            format!("{logged_by_unit}={name}"),
            format!("{logged_about_unit}={name}"),
            format!("SYSLOG_IDENTIFIER={stem}"),
            format!("SYSLOG_IDENTIFIER={stem}-start"),
        ]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(name: &str) -> FailedUnitInfo {
        FailedUnitInfo {
            name: name.to_string(),
            result: "exit-code".to_string(),
            status: 1,
            at: None,
        }
    }

    #[test]
    fn system_scope_matches_the_unit_and_its_own_output() {
        let failed = Failed::<false> {
            units: vec![unit("a.service")],
        };
        assert_eq!(
            failed.follow_log_args(),
            [
                "-x",
                "-b",
                "-n",
                "all",
                "-f",
                "_SYSTEMD_UNIT=a.service",
                "+",
                "UNIT=a.service",
                "+",
                "SYSLOG_IDENTIFIER=a",
                "+",
                "SYSLOG_IDENTIFIER=a-start",
            ]
        );
    }

    #[test]
    fn every_failed_unit_is_or_ed_into_one_stream() {
        let failed = Failed::<false> {
            units: vec![unit("a.service"), unit("b.timer")],
        };
        let args = failed.follow_log_args();
        assert_eq!(args.iter().filter(|arg| *arg == "+").count(), 7);
        assert!(args.contains(&"_SYSTEMD_UNIT=a.service".to_string()));
        assert!(args.contains(&"_SYSTEMD_UNIT=b.timer".to_string()));
        assert!(args.contains(&"SYSLOG_IDENTIFIER=b-start".to_string()));
        assert!(!args.starts_with(&["+".to_string()]));
        assert_ne!(args.last().map(String::as_str), Some("+"));
    }

    #[test]
    fn user_scope_matches_user_unit_fields() {
        let failed = Failed::<true> {
            units: vec![unit("a.service")],
        };
        assert_eq!(
            failed.follow_log_args(),
            [
                "--user",
                "-x",
                "-b",
                "-n",
                "all",
                "-f",
                "_SYSTEMD_USER_UNIT=a.service",
                "+",
                "USER_UNIT=a.service",
                "+",
                "SYSLOG_IDENTIFIER=a",
                "+",
                "SYSLOG_IDENTIFIER=a-start",
            ]
        );
    }

    #[test]
    fn without_units_falls_back_to_warnings() {
        let failed = Failed::<false> { units: Vec::new() };
        assert_eq!(
            failed.follow_log_args(),
            ["-x", "-b", "-f", "-p", "warning"]
        );
    }
}
