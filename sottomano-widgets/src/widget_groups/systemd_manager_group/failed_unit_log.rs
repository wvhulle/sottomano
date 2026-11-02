use std::{borrow::Cow, cmp::Reverse};

use chrono::{DateTime, Locale, Utc};
use iced::alignment::Horizontal;
use quick_xml::escape::escape;
use sottomano_config::Config;
use sottomano_sources::{
    desktop_notifications::notification_data::ActionTint,
    systemd_units_journal::{
        SystemdUnitScope,
        failed_units::{
            FailedUnitInfo, FailedUnitRestart, FailedUnits, ServiceOutcome, UNIT_FAILURE_PRIORITY,
        },
        journal_coredumps::{CoredumpEntry, CoredumpInspection},
    },
};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::{
        systemd_manager_group::{
            SystemdManagerGroup, SystemdManagerMessage, SystemdTarget, unit_scope_icon,
        },
        terminal_command::TerminalCommand,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{
            OngoingKey, OngoingToast, OngoingToastActionButton, definition_list, moment_text,
        },
        phosphor::Weight,
        value_cells::ValueCell,
    },
};

impl SystemdManagerGroup {}

pub(super) fn failed_units_item<'a>(
    failed: &'a FailedUnits,
    config: &'a Config,
    clock: Clock,
) -> GroupItem<'a, SystemdManagerMessage> {
    let open_log = |args| SystemdManagerMessage::OpenLog(TerminalCommand::journal(config, args));
    let failed_unit_toasts = failed.units.iter().map(|unit| {
        let open_unit_log = SystemdManagerMessage::OpenLog(TerminalCommand::journal(
            config,
            failed.scope.follow_units_log_args([unit.name.as_str()]),
        ));
        let inspect_coredump = |crash: &CoredumpEntry, inspection| {
            SystemdManagerMessage::InspectCoredump(TerminalCommand::running(
                config,
                crash.coredumpctl_args(inspection),
            ))
        };
        let coredump_buttons = unit.crash.iter().flat_map(|crash| {
            [
                OngoingToastActionButton {
                    button_label: "Info".to_owned(),
                    tint: ActionTint::Inspect,
                    on_button_pressed: inspect_coredump(crash, CoredumpInspection::Info),
                },
                OngoingToastActionButton {
                    button_label: "Debug".to_owned(),
                    tint: ActionTint::Inspect,
                    on_button_pressed: inspect_coredump(crash, CoredumpInspection::Debug),
                },
            ]
            .into_iter()
            .chain(
                crash
                    .stored_file
                    .as_ref()
                    .map(|file| OngoingToastActionButton {
                        button_label: "Copy path".to_owned(),
                        tint: ActionTint::Inspect,
                        on_button_pressed: SystemdManagerMessage::CopyToClipboard(
                            file.display().to_string(),
                        ),
                    }),
            )
        });
        OngoingToast {
            widget_name: failed.name(),
            key: OngoingKey::Reading(
                SystemdTarget::failed_unit(failed.scope, unit.name.clone()).into(),
            ),
            severity: UNIT_FAILURE_PRIORITY.severity(),
            summary_text: Cow::Owned(format!("{} failed", unit.name)),
            body: failure_summary(unit, clock.utc(), config.calendar.locale),
            progress: None,
            since: unit.at.unwrap_or(clock.wall),
            on_body_clicked: open_unit_log.clone(),
            action_buttons: [
                OngoingToastActionButton {
                    button_label: "Restart".to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: SystemdManagerMessage::RestartFailedUnit(
                        FailedUnitRestart {
                            per_user: failed.scope.per_user(),
                            unit: unit.name.clone(),
                        },
                    ),
                },
                OngoingToastActionButton {
                    button_label: "Journal".to_owned(),
                    tint: ActionTint::Inspect,
                    on_button_pressed: open_unit_log,
                },
                OngoingToastActionButton {
                    button_label: "Copy name".to_owned(),
                    tint: ActionTint::Inspect,
                    on_button_pressed: SystemdManagerMessage::CopyToClipboard(unit.name.clone()),
                },
            ]
            .into_iter()
            .chain(coredump_buttons)
            .collect(),
        }
    });
    GroupItem::reading(failed, SystemdTarget::failed_units(failed.scope))
        .on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            open_log(failed.follow_log_args()),
        )
        .ongoing_toasts(failed_unit_toasts)
}

fn service_outcome_reason(outcome: &ServiceOutcome) -> String {
    let status = outcome.exit_status;
    match outcome.result.as_str() {
        "exit-code" => format!("exited with code {status}"),
        "signal" => format!("killed by signal {status}"),
        "core-dump" => "crashed (core dumped)".to_owned(),
        "timeout" => "timed out".to_owned(),
        "watchdog" => "watchdog timeout".to_owned(),
        "oom-kill" => "killed (out of memory)".to_owned(),
        "start-limit-hit" => "too many restarts".to_owned(),
        "resources" => "could not start (resources)".to_owned(),
        other => other.to_owned(),
    }
}

fn failure_summary(unit: &FailedUnitInfo, now: DateTime<Utc>, locale: Locale) -> Option<String> {
    let reason = unit
        .service_outcome
        .as_ref()
        .map(|outcome| ("Reason", service_outcome_reason(outcome)));
    let failed_at = unit.at.map(|at| {
        (
            "Failed at",
            moment_text(DateTime::<Utc>::from(at), now, locale),
        )
    });
    let crash = unit.crash.iter().flat_map(|crash| {
        [
            ("Signal", crash.signal.to_string()),
            (
                "Process",
                format!(
                    "{} · PID {}",
                    escape(crash.executable.display().to_string()),
                    crash.pid
                ),
            ),
        ]
        .into_iter()
        .chain(crash.stored_file.as_ref().map(|file| {
            (
                "Core",
                format!("<tt>{}</tt>", escape(file.display().to_string())),
            )
        }))
    });
    let rows: Vec<(&str, String)> = reason.into_iter().chain(failed_at).chain(crash).collect();
    (!rows.is_empty()).then(|| definition_list(rows))
}

impl GroupItemReading for FailedUnits {
    fn visible_on_panel(&self) -> bool {
        !self.is_empty()
    }

    fn name(&self) -> &'static str {
        match self.scope {
            SystemdUnitScope::UserSession => "Failed user units",
            SystemdUnitScope::SystemWide => "Failed system units",
        }
    }

    fn icon(&self) -> IconGlyph {
        unit_scope_icon(self.scope)
            .with_weight(Weight::Regular)
            .into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.units.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        UNIT_FAILURE_PRIORITY.severity().into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let mut units = self.units.clone();
        units.sort_by_key(|unit| Reverse(unit.at));
        HoverCardBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Reason").align_x(Horizontal::Left),
                ColumnHeading::titled("Failed"),
            ],
            units.iter(),
        )
        .titled((
            self.name(),
            "Units in the failed state now; each toasts until it recovers.",
        ))])
    }
}

impl IntoTableRecord<2> for &FailedUnitInfo {
    fn into_table_record(self) -> TableRecord<2> {
        let reason = self
            .service_outcome
            .as_ref()
            .map_or(ValueCell::Blank, |outcome| {
                service_outcome_reason(outcome).into()
            });
        let failed = self.at.map_or(ValueCell::Blank, |at| ValueCell::Time {
            at: at.into(),
            format: LocalizedTimeFormat::DateAndTime,
        });
        TableRecord::new(self.name.clone(), [reason, failed])
            .level(UNIT_FAILURE_PRIORITY.severity())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_sources::systemd_units_journal::SystemdUnitScope;

    use super::*;
    use crate::hover_cards::hover_card_body::{Section, SectionBody};

    fn failed(name: &str, age: Option<Duration>) -> FailedUnitInfo {
        FailedUnitInfo {
            name: name.to_string(),
            service_outcome: Some(ServiceOutcome {
                result: "exit-code".to_string(),
                exit_status: 1,
            }),
            at: age.map(|age| Clock::real().wall - age),
            crash: None,
        }
    }

    fn section_keys(section: &Section) -> Vec<ValueCell> {
        let SectionBody::ColumnTable(table) = section.body() else {
            panic!("expected a table");
        };
        table.records().map(|record| record.key().clone()).collect()
    }

    #[test]
    fn units_list_newest_first_with_unknown_times_last() {
        let minutes = |count: u64| Some(Duration::from_secs(count * 60));
        let units = FailedUnits {
            units: vec![
                failed("undated.service", None),
                failed("old.service", minutes(180)),
                failed("fresh.service", minutes(1)),
                failed("older.service", minutes(300)),
                failed("recent.service", minutes(20)),
            ],
            scope: SystemdUnitScope::SystemWide,
        };
        let body = units.hover_card(Clock::real());
        let sections = body.listed_sections();
        assert_eq!(
            section_keys(&sections[0]),
            [
                "fresh.service",
                "recent.service",
                "old.service",
                "older.service",
                "undated.service"
            ]
            .map(ValueCell::from)
        );
    }
}
