use std::{borrow::Cow, cmp::Reverse};

use chrono::{DateTime, Local, Locale};

use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        source::{
            coredumps::CoredumpEntry,
            systemd_manager::failed_units::{
                Failed, FailedUnitInfo, FailedUnitRestart, ServiceOutcome, UNIT_FAILURE_PRIORITY,
            },
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
        reading_alert::{ReadingAlert, ReadingAlertActionButton},
        tooltip_body::{Cell, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl<S: ScopeAppearance> Failed<S> {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        let open_log =
            |args| SystemdManagerMessage::OpenLog(TerminalJournal::following(config, args));
        GroupItem::reading(self, S::FAILED_TARGET)
            .on_press(
                TerminalJournal::PRESS_HINT,
                open_log(self.follow_log_args()),
            )
            .alerts(self.unit_alerts(config))
    }

    fn unit_alerts<'a>(
        &'a self,
        config: &'a Config,
    ) -> impl Iterator<Item = ReadingAlert<SystemdManagerMessage>> + 'a {
        self.units.iter().map(move |unit| ReadingAlert {
            widget_name: self.name(),
            hover_target: S::failed_unit_target(unit.name.clone()).into(),
            severity: UNIT_FAILURE_PRIORITY.severity(),
            summary_text: Cow::Owned(format!("{} failed", unit.name)),
            body_text: failure_summary(unit, config.locale),
            on_body_clicked: SystemdManagerMessage::OpenLog(TerminalJournal::following(
                config,
                S::follow_units_log_args([unit.name.as_str()]),
            )),
            action_buttons: [ReadingAlertActionButton {
                button_label: "Restart".to_owned(),
                tint: ActionTint::Proceed,
                on_button_pressed: SystemdManagerMessage::RestartFailedUnit(FailedUnitRestart {
                    per_user: S::PER_USER,
                    unit: unit.name.clone(),
                }),
            }]
            .into_iter()
            .chain(
                unit.crash
                    .iter()
                    .flat_map(|crash| crash.action_buttons(config)),
            )
            .collect(),
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
    let outcome: Vec<String> = reason.into_iter().chain(failed_at).collect();
    let outcome_line = (!outcome.is_empty()).then(|| outcome.join(" · "));
    let lines: Vec<String> = outcome_line
        .into_iter()
        .chain(unit.crash.iter().flat_map(CoredumpEntry::labelled_fields))
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

impl<S: ScopeAppearance> BarWidget for Failed<S> {
    fn name(&self) -> &'static str {
        S::FAILED_LABEL
    }

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
        TooltipBody::sections(&[Section::column_table(
            (
                S::FAILED_LABEL,
                "Units in the failed state now; each toasts until it recovers.",
            ),
            ["Reason", "Failed"],
            units.iter().map(FailedUnitInfo::table_record),
        )])
    }
}

impl FailedUnitInfo {
    fn table_record(&self) -> TableRecord<2> {
        let reason = self
            .service_outcome
            .as_ref()
            .map(|outcome| outcome.description().into());
        let failed = self.at.map(|at| Cell::Time {
            at: at.into(),
            part: TimePart::DateAndTime,
        });
        TableRecord::new(self.name.clone(), [reason, failed])
            .level(UNIT_FAILURE_PRIORITY.severity())
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
            crash: None,
        }
    }

    fn section_keys(section: &Section) -> Vec<Cell> {
        let SectionBody::ColumnTable { records, .. } = section.body() else {
            panic!("expected a table");
        };
        records.iter().map(|record| record.key().clone()).collect()
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
