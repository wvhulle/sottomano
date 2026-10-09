use std::{borrow::Cow, cmp::Reverse};

use chrono::{DateTime, Local, Locale};

use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::systemd_manager::failed_units::{
            Failed, FailedUnitInfo, ServiceOutcome, UNIT_FAILURE_PRIORITY,
        },
    },
    widget_groups::{
        systemd_manager_group::{ScopeAppearance, SystemdManagerMessage},
        terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::TimePart,
        reading_alert::ReadingAlert,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, table_section},
        widget_state::WidgetState,
    },
};

impl<S: ScopeAppearance> Failed<S> {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem::reading(self, S::FAILED_TARGET).on_press(
            TerminalJournal::PRESS_HINT,
            SystemdManagerMessage::OpenLog(TerminalJournal::following(
                config,
                self.follow_log_args(),
            )),
        )
    }

    pub fn count_row(&self) -> Row {
        row(S::FAILED_LABEL, self.units.len().to_string())
    }

    pub fn unit_alerts<M>(
        &self,
        open_log: impl Fn(Vec<String>) -> M,
        locale: Locale,
    ) -> impl Iterator<Item = ReadingAlert<M>> {
        self.units.iter().map(move |unit| ReadingAlert {
            hover_target: S::failed_unit_target(unit.name.clone()),
            severity: UNIT_FAILURE_PRIORITY.severity(),
            summary_text: Cow::Owned(format!("{} failed", unit.name)),
            body_text: failure_summary(unit, locale),
            on_body_clicked: open_log(S::follow_units_log_args([unit.name.as_str()])),
            action_buttons: Vec::new(),
        })
    }
}

fn failure_summary(unit: &FailedUnitInfo, locale: Locale) -> Option<String> {
    let reason = unit
        .service_outcome
        .as_ref()
        .map(ServiceOutcome::description);
    let failed_at = unit
        .at
        .map(|at| TimePart::TimeOfDay.format(&DateTime::<Local>::from(at), locale));
    let parts: Vec<String> = reason.into_iter().chain(failed_at).collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
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
        UNIT_FAILURE_PRIORITY.severity().into()
    }

    fn tooltip(&self) -> TooltipBody {
        let mut units = self.units.clone();
        units.sort_by_key(|unit| Reverse(unit.at));
        TooltipBody::Sections(vec![table_section(S::FAILED_LABEL, units).with_caption(
            "Units in the failed state now; each toasts until it recovers.",
        )])
    }
}

impl TableRow for FailedUnitInfo {
    const HEADINGS: &'static [&'static str] = &["Reason", "Failed"];

    fn key(&self) -> Cell {
        self.name.clone().into()
    }

    fn cells(&self) -> Vec<Cell> {
        let reason = self
            .service_outcome
            .as_ref()
            .map_or(Cell::Missing, |outcome| outcome.description().into());
        let failed = self.at.map_or(Cell::Missing, |at| Cell::Time {
            at: at.into(),
            part: TimePart::DateAndTime,
        });
        vec![reason, failed]
    }

    fn level(&self) -> Severity {
        UNIT_FAILURE_PRIORITY.severity()
    }
}

impl ServiceOutcome {
    fn description(&self) -> String {
        let status = self.exit_status;
        match self.result.as_str() {
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
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::{
        data::source::systemd_manager::System,
        widgets::tooltip_body::{Section, SectionBody},
    };

    fn failed(name: &str, age: Option<Duration>) -> FailedUnitInfo {
        FailedUnitInfo {
            name: name.to_string(),
            service_outcome: Some(ServiceOutcome {
                result: "exit-code".to_string(),
                exit_status: 1,
            }),
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
