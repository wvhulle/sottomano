use std::borrow::Cow;

use chrono::{DateTime, Local, Locale};

use crate::{
    config::Config,
    data::{
        severity::Severity,
        source::systemd_manager::running_jobs::{SlowJob, SlowJobs},
    },
    widget_groups::{
        systemd_manager_group::{ScopeAppearance, SystemdManagerMessage},
        terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::{GroupItem, Press},
        localized_time::TimePart,
        phosphor::Phosphor,
        reading_alert::ReadingAlert,
        tooltip_body::{Cell, Section, TableRow, TooltipBody},
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
}

impl TableRow for SlowJob {
    const HEADINGS: &'static [&'static str] = &["Job", "Since"];

    fn key(&self) -> Cell {
        self.unit.clone().into()
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            self.job_type.clone().into(),
            Cell::Time {
                at: self.started_at.into(),
                part: TimePart::TimeOfDay,
            },
        ]
    }

    fn level(&self) -> Severity {
        Severity::Warning
    }
}

impl<S: ScopeAppearance> SlowJobs<S> {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        let open_log =
            |args| SystemdManagerMessage::OpenLog(TerminalJournal::following(config, args));
        GroupItem {
            on_press: Some(Press {
                does: TerminalJournal::PRESS_HINT,
                message: open_log(self.follow_log_args()),
            }),
            alerts: self.job_alerts(open_log, config.locale).collect(),
            ..GroupItem::reading(self, S::SLOW_JOBS_TARGET)
        }
    }

    fn job_alerts<M>(
        &self,
        open_log: impl Fn(Vec<String>) -> M,
        locale: Locale,
    ) -> impl Iterator<Item = ReadingAlert<M>> {
        self.jobs.iter().map(move |job| ReadingAlert {
            hover_target: S::slow_job_target(job.unit.clone()),
            severity: Severity::Warning,
            summary_text: Cow::Owned(format!("{} {} slowly", job.unit, job.activity())),
            body_text: Some(format!(
                "since {} · {}",
                TimePart::TimeOfDay.format(&DateTime::<Local>::from(job.started_at), locale),
                S::SCOPE_NAME
            )),
            on_body_clicked: open_log(S::follow_units_log_args([job.unit.as_str()])),
            action_buttons: Vec::new(),
        })
    }
}

impl<S: ScopeAppearance> BarWidget for SlowJobs<S> {
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
        TooltipBody::sections(&[Section::table(
            (
                S::SLOW_JOBS_LABEL,
                "Start or stop jobs still running after slow_job_secs; each also raises a toast.",
            ),
            &self.jobs,
        )])
    }
}
