use std::cmp::Reverse;

use crate::{
    data::source::{
        journal::FailedUnitInfo,
        systemd_failed::{Failed, JournalScope, System, User},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, TableRow, TooltipBody, table_section},
        widget_state::{Level, WidgetState},
    },
};

pub trait ScopeAppearance: JournalScope {
    const ICON: Phosphor;
    const LABEL: &'static str;

    fn severity(recent: bool) -> Level;
}

impl ScopeAppearance for User {
    const ICON: Phosphor = Phosphor::User(Weight::Regular);
    const LABEL: &'static str = "Failed user units";

    fn severity(_recent: bool) -> Level {
        Level::Warning
    }
}

impl ScopeAppearance for System {
    const ICON: Phosphor = Phosphor::HardDrives(Weight::Regular);
    const LABEL: &'static str = "Failed system units";

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
        S::ICON.with_weight(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.units.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        S::severity(self.units.iter().any(Self::recent)).into()
    }

    fn tooltip(&self) -> TooltipBody {
        let mut units: Vec<&FailedUnitInfo> = self.units.iter().collect();
        units.sort_by_key(|unit| Reverse(unit.at));
        TooltipBody::Sections(vec![table_section(
            S::LABEL,
            units.into_iter().map(|unit| FailedUnitRow {
                unit,
                level: S::severity(Failed::<S>::recent(unit)),
            }),
        )])
    }
}

struct FailedUnitRow<'a> {
    unit: &'a FailedUnitInfo,
    level: Level,
}

impl TableRow for FailedUnitRow<'_> {
    const HEADINGS: &'static [&'static str] = &["Reason", "Failed"];

    fn key(&self) -> Cell {
        self.unit.name.clone().into()
    }

    fn cells(&self) -> Vec<Cell> {
        let failed = self.unit.at.map_or_else(
            || "unknown".into(),
            |at| Cell::Time {
                at: at.into(),
                part: TimePart::DateAndTime,
            },
        );
        vec![
            describe_failure(&self.unit.result, self.unit.status).into(),
            failed,
        ]
    }

    fn level(&self) -> Level {
        self.level
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
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::widgets::tooltip_body::{Section, SectionBody};

    fn failed(name: &str, age: Option<Duration>) -> FailedUnitInfo {
        FailedUnitInfo {
            name: name.to_string(),
            result: "exit-code".to_string(),
            status: 1,
            at: age.map(|age| SystemTime::now() - age),
        }
    }

    fn section_keys(section: &Section) -> Vec<Cell> {
        let SectionBody::Columns { rows, .. } = &section.body else {
            panic!("expected a table");
        };
        rows.iter().map(|row| row.key.clone()).collect()
    }

    #[test]
    fn units_list_newest_first_with_unknown_times_last() {
        let minutes = |count: u64| Some(Duration::from_secs(count * 60));
        let units = Failed::<System>::from_units(vec![
            failed("undated.service", None),
            failed("old.service", minutes(180)),
            failed("fresh.service", minutes(1)),
            failed("older.service", minutes(300)),
            failed("recent.service", minutes(20)),
        ])
        .expect("units present");
        let TooltipBody::Sections(sections) = units.tooltip() else {
            panic!("expected sections");
        };
        assert_eq!(
            section_keys(&sections[0]),
            [
                "fresh.service",
                "recent.service",
                "old.service",
                "older.service",
                "undated.service"
            ]
            .map(Cell::from)
        );
    }
}
