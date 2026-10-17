use std::borrow::Cow;

use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;

use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::systemd_manager::running_jobs::{SlowJob, SlowJobs},
    },
    widget_groups::{
        systemd_manager_group::{ScopeAppearance, SystemdManagerMessage},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        group_item::GroupItem,
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{OngoingBody, OngoingKey, OngoingToast},
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl SlowJob {
    fn activity(&self) -> Cow<'_, str> {
        match self.job_type.as_str() {
            "start" => Cow::Borrowed("starting"),
            "stop" => Cow::Borrowed("stopping"),
            "restart" => Cow::Borrowed("restarting"),
            "reload" => Cow::Borrowed("reloading"),
            other => Cow::Owned(format!("running its {other} job")),
        }
    }

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

impl<S: ScopeAppearance> SlowJobs<S> {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        let open_log =
            |args| SystemdManagerMessage::OpenLog(TerminalCommand::journal(config, args));
        GroupItem::reading(self, S::SLOW_JOBS_TARGET)
            .on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                open_log(self.follow_log_args()),
            )
            .ongoing_toasts(self.slow_job_toasts(open_log, config.calendar.locale))
    }

    fn slow_job_toasts<M>(
        &self,
        open_log: impl Fn(Vec<String>) -> M,
        locale: Locale,
    ) -> impl Iterator<Item = OngoingToast<M>> {
        self.jobs.iter().map(move |job| OngoingToast {
            widget_name: self.name(),
            key: OngoingKey::Reading(S::slow_job_target(job.unit.clone()).into()),
            severity: Severity::Warning,
            summary_text: Cow::Owned(format!("{} {} slowly", job.unit, job.activity())),
            body: OngoingBody::Text(format!(
                "since {} · {}",
                LocalizedTimeFormat::TimeOfDay
                    .format(&DateTime::<Local>::from(job.started_at), locale),
                S::SCOPE_NAME
            )),
            progress: None,
            since: job.started_at,
            on_body_clicked: open_log(S::follow_units_log_args([job.unit.as_str()])),
            action_buttons: Vec::new(),
        })
    }
}

impl<S: ScopeAppearance> Occasional for SlowJobs<S> {
    fn present(&self) -> bool {
        !self.is_empty()
    }
}

impl<S: ScopeAppearance> BarWidget for SlowJobs<S> {
    fn name(&self) -> &'static str {
        S::SLOW_JOBS_LABEL
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
            S::SLOW_JOBS_LABEL,
            "Start or stop jobs still running after slow_job_secs; each also raises a toast.",
        ))])
    }
}
