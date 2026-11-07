use std::{borrow::Cow, time::Duration};

use chrono::{DateTime, Locale, Utc};
use sottomano_desktop_sources::{
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskUsage, TaskWebLog,
    },
    podman_container_engine::libpod_running_containers::EngineReadError,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::process_usage_cells::{approximate_memory_text, cpu_share_quantity},
    widgets::{
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CiRunnerKind {
    Woodpecker,
    Forgejo,
}

impl CiRunnerKind {
    pub(super) const fn runner_icon(self) -> IconGlyph {
        match self {
            CiRunnerKind::Woodpecker => IconGlyph::Phosphor(Phosphor::Bird(Weight::Regular)),
            CiRunnerKind::Forgejo => IconGlyph::SimpleIcon(SimpleIcon::Forgejo),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct WatchedCiRunner<'a> {
    pub kind: CiRunnerKind,
    pub runner_label: &'a str,
    pub runner: &'a CiRunnerTasks,
}

impl WatchedCiRunner<'_> {
    fn status_text(&self) -> String {
        access_failure_text(&self.runner.access).map_or_else(
            || running_count_text(self.runner.tasks.len()),
            str::to_owned,
        )
    }

    fn failure_section(&self) -> Option<Section> {
        match &self.runner.access {
            RunnerAccess::Complete => None,
            RunnerAccess::EngineUnreadable(failure) => Some(Section::error(failure)),
            RunnerAccess::AgentUnreachable(failure) => Some(Section::error(failure)),
        }
    }
}

fn running_count_text(running_count: usize) -> String {
    format!("{running_count} running")
}

fn access_failure_text(access: &RunnerAccess) -> Option<&'static str> {
    match access {
        RunnerAccess::Complete => None,
        RunnerAccess::EngineUnreadable(EngineReadError::SocketDenied) => {
            Some("no access: socket denied")
        }
        RunnerAccess::EngineUnreadable(EngineReadError::SocketMissing) => {
            Some("engine socket missing")
        }
        RunnerAccess::EngineUnreadable(_) => Some("engine unreadable"),
        RunnerAccess::AgentUnreachable(_) => Some("agent unreachable"),
    }
}

pub(super) struct CiRunnerFailureRow<'a> {
    pub watched: WatchedCiRunner<'a>,
}

impl GroupItemReading for CiRunnerFailureRow<'_> {
    fn name(&self) -> &'static str {
        "CI runner"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.watched.runner_label.to_owned().into()
    }

    fn icon(&self) -> IconGlyph {
        self.watched.kind.runner_icon()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.watched.status_text())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(self.watched.failure_section())
    }
}

impl<'a> CiRunnerFailureRow<'a> {
    pub(super) fn of_failing(watched: WatchedCiRunner<'a>) -> Option<Self> {
        match watched.runner.access {
            RunnerAccess::Complete => None,
            RunnerAccess::EngineUnreadable(_) | RunnerAccess::AgentUnreachable(_) => {
                Some(Self { watched })
            }
        }
    }
}

fn task_row_name(task: &CiTask) -> String {
    match task.repository.short_name() {
        Some(short_name) => format!("{short_name} {}", task.run_label),
        None => format!("task {}", task.run_label),
    }
}

fn task_full_name(task: &CiTask) -> String {
    match &task.repository {
        TaskRepository::Named(full_name) => format!("{full_name} {}", task.run_label),
        TaskRepository::NotYetKnown => format!("task {}", task.run_label),
    }
}

fn elapsed_of(task: &CiTask, clock: Clock) -> Duration {
    clock
        .wall
        .duration_since(task.started_at)
        .unwrap_or_default()
}

fn web_log_entry(web_log: &TaskWebLog) -> Option<LabeledValue> {
    match web_log {
        TaskWebLog::Linked(url) => Some(LabeledValue::new(
            "Log",
            ValueCell::Link {
                label: "open in browser".to_owned(),
                url: url.clone(),
            },
        )),
        TaskWebLog::NotKnown => None,
    }
}

pub(super) struct CiTaskRow<'a> {
    pub kind: CiRunnerKind,
    pub task: &'a CiTask,
}

impl GroupItemReading for CiTaskRow<'_> {
    fn name(&self) -> &'static str {
        "CI task"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        task_row_name(self.task).into()
    }

    fn icon(&self) -> IconGlyph {
        self.kind.runner_icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(duration_text(elapsed_of(self.task, clock), locale))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Live
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let joined_or_none = |values: &[String]| {
            if values.is_empty() {
                ValueCell::Hint("no container right now".to_owned())
            } else {
                values.join(", ").into()
            }
        };
        let deadline = match self.task.deadline {
            TaskDeadline::NotExposed => None,
            TaskDeadline::At(deadline) => Some(LabeledValue::new(
                "Timeout",
                ValueCell::RelativeMoment(DateTime::<Utc>::from(deadline)),
            )),
        };
        let usage = match self.task.usage {
            TaskUsage::NoContainerNow => Vec::new(),
            TaskUsage::Measured {
                cpu_load,
                resident_memory,
            } => cpu_share_quantity(cpu_load)
                .map(|cpu_share| LabeledValue::new("CPU load", cpu_share))
                .into_iter()
                .chain([LabeledValue::new(
                    "Memory",
                    approximate_memory_text(resident_memory),
                )])
                .collect(),
        };
        HoverCardBody::sections([Section::dictionary(
            [
                LabeledValue::new("Task", task_full_name(self.task)),
                LabeledValue::new("Running", joined_or_none(&self.task.activities)),
                LabeledValue::new("Image", joined_or_none(&self.task.images)),
            ]
            .into_iter()
            .chain(usage)
            .chain(deadline)
            .chain(web_log_entry(&self.task.web_log)),
        )])
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use bytesize::ByteSize;
    use sottomano_desktop_sources::system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad;
    use uom::si::{f64::Ratio, ratio::percent};

    use super::*;

    fn task(repository: TaskRepository, run_label: &str) -> CiTask {
        CiTask {
            repository,
            run_label: run_label.to_owned(),
            started_at: UNIX_EPOCH,
            deadline: TaskDeadline::NotExposed,
            activities: Vec::new(),
            images: Vec::new(),
            usage: TaskUsage::Measured {
                cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(1.0)),
                resident_memory: ByteSize::mib(1),
            },
            web_log: TaskWebLog::NotKnown,
        }
    }

    fn runner_with(task_count: usize, access: RunnerAccess) -> CiRunnerTasks {
        CiRunnerTasks {
            tasks: (0..task_count)
                .map(|task_index| task(TaskRepository::NotYetKnown, &task_index.to_string()))
                .collect(),
            access,
        }
    }

    #[test]
    fn the_row_shows_the_short_repo_and_the_card_the_full_one() {
        let named = task(
            TaskRepository::Named("wvhulle/sottomano".to_owned()),
            "#185",
        );
        assert_eq!(task_row_name(&named), "sottomano #185");
        assert_eq!(task_full_name(&named), "wvhulle/sottomano #185");
    }

    #[test]
    fn an_unmatched_task_is_named_by_its_id() {
        let unmatched = task(TaskRepository::NotYetKnown, "11282424");
        assert_eq!(task_row_name(&unmatched), "task 11282424");
    }

    #[test]
    fn a_task_started_in_the_future_has_no_elapsed_time() {
        let future = CiTask {
            started_at: UNIX_EPOCH + Duration::from_secs(100),
            ..task(TaskRepository::NotYetKnown, "7")
        };
        let clock = Clock {
            wall: UNIX_EPOCH,
            ..Clock::real()
        };
        assert_eq!(elapsed_of(&future, clock), Duration::ZERO);
    }

    #[test]
    fn each_runner_kind_shows_its_forge_icon() {
        assert_eq!(
            CiRunnerKind::Woodpecker.runner_icon(),
            IconGlyph::Phosphor(Phosphor::Bird(Weight::Regular))
        );
        assert_eq!(
            CiRunnerKind::Forgejo.runner_icon(),
            IconGlyph::SimpleIcon(SimpleIcon::Forgejo)
        );
    }

    #[test]
    fn a_denied_engine_is_explained_on_the_row() {
        assert_eq!(
            access_failure_text(&RunnerAccess::EngineUnreadable(
                EngineReadError::SocketDenied
            )),
            Some("no access: socket denied")
        );
        assert_eq!(access_failure_text(&RunnerAccess::Complete), None);
    }

    #[test]
    fn only_a_failing_runner_gets_its_own_row() {
        let healthy_runner = runner_with(1, RunnerAccess::Complete);
        let denied_runner = runner_with(
            0,
            RunnerAccess::EngineUnreadable(EngineReadError::SocketDenied),
        );
        let watched = |runner| WatchedCiRunner {
            kind: CiRunnerKind::Forgejo,
            runner_label: "codeberg",
            runner,
        };
        assert!(CiRunnerFailureRow::of_failing(watched(&healthy_runner)).is_none());
        assert!(CiRunnerFailureRow::of_failing(watched(&denied_runner)).is_some());
    }

    #[test]
    fn a_linked_run_offers_its_web_log() {
        let linked = web_log_entry(&TaskWebLog::Linked(
            "https://codeberg.org/wvhulle/bindrune/actions".to_owned(),
        ))
        .expect("a linked run has a log entry");
        assert_eq!(
            linked.value(),
            &ValueCell::Link {
                label: "open in browser".to_owned(),
                url: "https://codeberg.org/wvhulle/bindrune/actions".to_owned(),
            }
        );
        assert!(web_log_entry(&TaskWebLog::NotKnown).is_none());
    }
}
