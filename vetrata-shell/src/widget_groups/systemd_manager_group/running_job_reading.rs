use std::borrow::Cow;

use chrono::{DateTime, Local};
use iced::alignment::Horizontal;
use vetrata_config::{Config, severity::Severity};
use vetrata_sources::systemd_manager::running_jobs::{SlowJob, SlowJobs};

use crate::{
    widget_groups::{
        systemd_manager_group::{SystemdManagerMessage, UnitScopePresentation},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::{ConfiguredGroupItem, GroupItem},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{OngoingBody, OngoingKey, OngoingToast},
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

impl TableRow<2> for SlowJob {
    fn table_record(&self) -> TableRecord<2> {
        TableRecord::new(
            self.unit.clone(),
            [
                Some(self.job_type.clone().into()),
                Some(Cell::Time {
                    at: self.started_at.into(),
                    format: LocalizedTimeFormat::TimeOfDay,
                }),
            ],
        )
        .level(Severity::Warning)
    }
}

impl ConfiguredGroupItem<SystemdManagerMessage> for SlowJobs {
    fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        let open_log =
            |args| SystemdManagerMessage::OpenLog(TerminalCommand::journal(config, args));
        let locale = config.calendar.locale;
        let slow_job_toasts = self.jobs.iter().map(|job| {
            let activity = match job.job_type.as_str() {
                "start" => Cow::Borrowed("starting"),
                "stop" => Cow::Borrowed("stopping"),
                "restart" => Cow::Borrowed("restarting"),
                "reload" => Cow::Borrowed("reloading"),
                other => Cow::Owned(format!("running its {other} job")),
            };
            OngoingToast {
                widget_name: self.name(),
                key: OngoingKey::Reading(self.scope.slow_job_target(job.unit.clone()).into()),
                severity: Severity::Warning,
                summary_text: Cow::Owned(format!("{} {activity} slowly", job.unit)),
                body: OngoingBody::Text(format!(
                    "since {} · {}",
                    LocalizedTimeFormat::TimeOfDay
                        .format(&DateTime::<Local>::from(job.started_at), locale),
                    self.scope.scope_name()
                )),
                progress: None,
                since: job.started_at,
                on_body_clicked: open_log(self.scope.follow_units_log_args([job.unit.as_str()])),
                action_buttons: Vec::new(),
            }
        });
        GroupItem::reading(self, self.scope.slow_jobs_target())
            .on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                open_log(self.follow_log_args()),
            )
            .ongoing_toasts(slow_job_toasts)
    }
}

impl BarWidget for SlowJobs {
    fn visible_in_bar(&self) -> bool {
        !self.is_empty()
    }

    fn name(&self) -> &'static str {
        self.scope.slow_jobs_label()
    }

    fn icon(&self) -> Glyph {
        Phosphor::HourglassMedium(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.jobs.len()).ok()
    }

    fn state(&self) -> WidgetState {
        WidgetState::Warning
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Job").align_x(Horizontal::Left),
                ColumnHeading::new("Since"),
            ],
            self.jobs.iter().map(SlowJob::table_record),
        )
        .titled((
            self.scope.slow_jobs_label(),
            "Start or stop jobs still running after slow_job_secs; each also raises a toast.",
        ))])
    }
}
