use std::borrow::Cow;

use chrono::{DateTime, Utc};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::{
    SystemdUnitScope,
    slow_running_jobs::{SlowJob, SlowJobs},
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::ReadoutBadge,
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
        terminal_command::TerminalCommand,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    panel_menus::services_menu::systemd_manager_group::{
        SystemdManagerInteraction, unit_toast_key,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{
            NotificationBody, NotificationProgress, OngoingToast, definition_list, moment_text,
        },
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::ValueCell,
    },
};

const SLOW_JOBS_GLYPH: Phosphor = Phosphor::HourglassMedium(Weight::Regular);

impl From<&SlowJob> for TableRecord<2> {
    fn from(slow_job: &SlowJob) -> Self {
        TableRecord::with_cells(
            slow_job.unit.clone(),
            [
                slow_job.job_type.clone().into(),
                ValueCell::Time {
                    at: slow_job.started_at.into(),
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ],
        )
        .level(Severity::Warning)
    }
}

pub(super) fn wired_slow_jobs_indicator<'view>(
    jobs: &'view SlowJobs,
    config: &'view Config,
    clock: Clock,
) -> GroupIndicator<'view> {
    let open_log =
        |args| SystemdManagerInteraction::OpenLog(TerminalCommand::journal(config, args));
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
            widget_name: slow_jobs_name(jobs),
            key: unit_toast_key(slow_jobs_name(jobs), job.unit.clone()),
            severity: Severity::Warning,
            summary_text: Cow::Owned(format!("{} {activity} slowly", job.unit)),
            body: NotificationBody::Text(definition_list([
                (
                    "Since",
                    moment_text(DateTime::<Utc>::from(job.started_at), clock.utc(), locale),
                ),
                ("Scope", scope_name.to_owned()),
            ])),
            progress: NotificationProgress::Unmeasured,
            since: job.started_at,
            on_body_clicked: open_log(jobs.scope.follow_units_log_args([job.unit.as_str()])),
            action_buttons: Vec::new(),
        }
    });
    slow_jobs_indicator(jobs)
        .on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            open_log(jobs.follow_log_args()),
        )
        .ongoing_toasts(slow_job_toasts)
}

pub(super) fn slow_jobs_panel_visibility(jobs: &SlowJobs) -> PanelVisibility {
    if jobs.jobs.is_empty() {
        PanelVisibility::Hidden
    } else {
        PanelVisibility::Shown
    }
}

pub(super) fn slow_jobs_name(jobs: &SlowJobs) -> Cow<'static, str> {
    Cow::Borrowed(match jobs.scope {
        SystemdUnitScope::UserSession => "Slow user jobs",
        SystemdUnitScope::SystemWide => "Slow system jobs",
    })
}

pub(super) fn slow_jobs_hover_card(jobs: &SlowJobs) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            slow_jobs_name(jobs),
            "Start or stop jobs still running after slow_job_secs; each also raises a toast.",
        ),
        headed_table(
            [
                ColumnHeading::titled("Job").align_x(Horizontal::Left),
                ColumnHeading::titled("Since"),
            ],
            jobs.jobs.iter(),
        ),
    )]
}

pub(super) fn slow_jobs_indicator(jobs: &SlowJobs) -> GroupIndicator<'_> {
    let base = GroupIndicator::glyph_only(
        slow_jobs_name(jobs),
        Phosphor::HourglassMedium(Weight::Regular).into(),
        IndicatorState::Warning,
    )
    .with_hover_card(Box::new(move |_| slow_jobs_hover_card(jobs)));
    let badge = match jobs.jobs.len() {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    base.with_badge(badge)
        .with_panel_visibility(slow_jobs_panel_visibility(jobs))
}

fn slow_jobs_entry<'view>(
    snapshot: &'view SubscribedSnapshot<SlowJobs>,
    jobs: &'view SlowJobs,
    config: &'view Config,
    clock: Clock,
) -> GroupIndicator<'view> {
    wired_slow_jobs_indicator(jobs, config, clock).with_hover_card(Box::new(move |_| {
        snapshot.sections_described_by(slow_jobs_hover_card)
    }))
}

pub(super) fn slow_jobs_listing<'view>(
    slow_jobs: &'view SubscribedSnapshot<SlowJobs>,
    config: &'view Config,
    clock: Clock,
) -> IndicatorListing<'view> {
    slow_jobs.indicator_listing(
        |held_snapshot| match slow_jobs_panel_visibility(held_snapshot) {
            PanelVisibility::Shown => {
                vec![slow_jobs_entry(slow_jobs, held_snapshot, config, clock)]
            }
            PanelVisibility::Hidden => Vec::new(),
        },
        SLOW_JOBS_GLYPH.into(),
    )
}
