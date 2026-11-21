use std::{borrow::Cow, cmp::Reverse};

use chrono::{DateTime, Locale, Utc};
use iced::alignment::Horizontal;
use quick_xml::escape::escape;
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::ActionTint,
    systemd_units_journal::{
        SystemdUnitScope,
        failed_units::{
            FailedUnitInfo, FailedUnitRestart, FailedUnits, ServiceOutcome, UNIT_FAILURE_PRIORITY,
        },
        journal_coredumps::{CoredumpEntry, CoredumpInspection},
    },
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    glyph_icons::phosphor::Weight,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::ReadoutBadge,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTable, SectionTitle, headed_table},
    indicator_groups::{
        systemd_manager_group::{SystemdManagerInteraction, unit_scope_icon, unit_toast_key},
        terminal_command::TerminalCommand,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{OngoingToast, OngoingToastActionButton, definition_list, moment_text},
        value_cells::ValueCell,
    },
};

pub(super) fn wired_failed_units_indicator<'a>(
    failed: &'a FailedUnits,
    config: &'a Config,
    clock: Clock,
) -> GroupIndicator<'a> {
    let open_log =
        |args| SystemdManagerInteraction::OpenLog(TerminalCommand::journal(config, args));
    let failed_unit_toasts = failed.units.iter().map(|unit| {
        let open_unit_log = SystemdManagerInteraction::OpenLog(TerminalCommand::journal(
            config,
            failed.scope.follow_units_log_args([unit.name.as_str()]),
        ));
        let inspect_coredump = |crash: &CoredumpEntry, inspection| {
            SystemdManagerInteraction::InspectCoredump(TerminalCommand::running(
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
                        on_button_pressed: SystemdManagerInteraction::CopyToClipboard(
                            file.display().to_string(),
                        ),
                    }),
            )
        });
        OngoingToast {
            widget_name: failed_units_name(failed),
            key: unit_toast_key(failed_units_name(failed), unit.name.clone()),
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
                    on_button_pressed: SystemdManagerInteraction::RestartFailedUnit(
                        FailedUnitRestart {
                            unit_scope: failed.scope,
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
                    on_button_pressed: SystemdManagerInteraction::CopyToClipboard(
                        unit.name.clone(),
                    ),
                },
            ]
            .into_iter()
            .chain(coredump_buttons)
            .collect(),
        }
    });
    failed_units_indicator(failed)
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
                escape(crash.executable.display().to_string()).into_owned(),
            ),
            ("PID", crash.pid.to_string()),
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

pub(super) fn failed_units_panel_visibility(failed: &FailedUnits) -> PanelVisibility {
    if failed.units.is_empty() {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    }
}

pub(super) fn failed_units_name(failed: &FailedUnits) -> Cow<'static, str> {
    Cow::Borrowed(match failed.scope {
        SystemdUnitScope::UserSession => "Failed user units",
        SystemdUnitScope::SystemWide => "Failed system units",
    })
}

pub(super) fn failed_units_state() -> IndicatorState {
    UNIT_FAILURE_PRIORITY.severity().into()
}

pub(super) fn failed_units_hover_card(failed: &FailedUnits) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Units",
            "Units in the failed state now; each toasts until it recovers.",
        ),
        failed_unit_table(&failed.units),
    )]
}

pub(super) fn failed_units_indicator(failed: &FailedUnits) -> GroupIndicator<'_> {
    let base = GroupIndicator::glyph_only(
        failed_units_name(failed),
        unit_scope_icon(failed.scope)
            .with_weight(Weight::Regular)
            .into(),
        failed_units_state(),
    )
    .with_hover_card(Box::new(move |_| failed_units_hover_card(failed)));
    let badge = match failed.units.len() {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    base.with_badge(badge)
        .with_panel_visibility(failed_units_panel_visibility(failed))
}

fn failed_unit_table(units: &[FailedUnitInfo]) -> SectionTable {
    let mut newest_first = units.to_vec();
    newest_first.sort_by_key(|unit| Reverse(unit.at));
    headed_table(
        [
            ColumnHeading::titled("Reason").align_x(Horizontal::Left),
            ColumnHeading::titled("Failed"),
        ],
        newest_first.iter(),
    )
}

impl From<&FailedUnitInfo> for TableRecord<2> {
    fn from(failed_unit_info: &FailedUnitInfo) -> Self {
        let reason = failed_unit_info
            .service_outcome
            .as_ref()
            .map_or(ValueCell::Blank, |outcome| {
                service_outcome_reason(outcome).into()
            });
        let failed = failed_unit_info
            .at
            .map_or(ValueCell::Blank, |at| ValueCell::Time {
                at: at.into(),
                format: LocalizedTimeFormat::DateAndTime,
            });
        TableRecord::with_cells(failed_unit_info.name.clone(), [reason, failed])
            .level(UNIT_FAILURE_PRIORITY.severity())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::systemd_units_journal::SystemdUnitScope;

    use super::*;

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

    fn table_keys(table: &SectionTable) -> Vec<ValueCell> {
        table.records().map(|record| record.key.clone()).collect()
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
        assert_eq!(
            table_keys(&failed_unit_table(&units.units)),
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
