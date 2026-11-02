use std::borrow::Cow;

use chrono::{DateTime, Utc};
use iced::alignment::Horizontal;
use sottomano_config::{Config, severity::Severity};
use sottomano_sources::systemd_units_journal::{
    SystemdUnitScope,
    slow_running_jobs::{SlowJob, SlowJobs},
};

use crate::{
    clock::Clock,
    widget_groups::{
        systemd_manager_group::{SystemdManagerGroup, SystemdManagerMessage, SystemdTarget},
        terminal_command::TerminalCommand,
    },
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{OngoingKey, OngoingToast, definition_list, moment_text},
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        value_cells::ValueCell,
    },
};

impl IntoTableRecord<2> for &SlowJob {
    fn into_table_record(self) -> TableRecord<2> {
        TableRecord::new(
            self.unit.clone(),
            [
                self.job_type.clone().into(),
                ValueCell::Time {
                    at: self.started_at.into(),
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ],
        )
        .level(Severity::Warning)
    }
}

impl SystemdManagerGroup {}

pub(super) fn slow_jobs_item<'a>(
    jobs: &'a SlowJobs,
    config: &'a Config,
    clock: Clock,
) -> GroupItem<'a, SystemdManagerMessage> {
    let open_log = |args| SystemdManagerMessage::OpenLog(TerminalCommand::journal(config, args));
    let locale = config.calendar.locale;
    let slow_job_toasts = jobs.jobs.iter().map(|job| {
        let activity = match job.job_type.as_str() {
            "start" => Cow::Borrowed("starting"),
            "stop" => Cow::Borrowed("stopping"),
            "restart" => Cow::Borrowed("restarting"),
            "reload" => Cow::Borrowed("reloading"),
            other => Cow::Owned(format!("running its {other} job")),
        };
        let scope_name = match jobs.scope {
            SystemdUnitScope::UserSession => "user",
            SystemdUnitScope::SystemWide => "system",
        };
        OngoingToast {
            widget_name: jobs.name(),
            key: OngoingKey::Reading(SystemdTarget::slow_job(jobs.scope, job.unit.clone()).into()),
            severity: Severity::Warning,
            summary_text: Cow::Owned(format!("{} {activity} slowly", job.unit)),
            body: Some(definition_list([
                (
                    "Since",
                    moment_text(DateTime::<Utc>::from(job.started_at), clock.utc(), locale),
                ),
                ("Scope", scope_name.to_owned()),
            ])),
            progress: None,
            since: job.started_at,
            on_body_clicked: open_log(jobs.scope.follow_units_log_args([job.unit.as_str()])),
            action_buttons: Vec::new(),
        }
    });
    GroupItem::reading(jobs, SystemdTarget::slow_jobs(jobs.scope))
        .on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            open_log(jobs.follow_log_args()),
        )
        .ongoing_toasts(slow_job_toasts)
}

impl GroupItemReading for SlowJobs {
    fn visible_on_panel(&self) -> bool {
        !self.is_empty()
    }

    fn name(&self) -> &'static str {
        match self.scope {
            SystemdUnitScope::UserSession => "Slow user jobs",
            SystemdUnitScope::SystemWide => "Slow system jobs",
        }
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::HourglassMedium(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.jobs.len()).ok()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Warning
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Job").align_x(Horizontal::Left),
                ColumnHeading::titled("Since"),
            ],
            self.jobs.iter(),
        )
        .titled((
            self.name(),
            "Start or stop jobs still running after slow_job_secs; each also raises a toast.",
        ))])
    }
}
