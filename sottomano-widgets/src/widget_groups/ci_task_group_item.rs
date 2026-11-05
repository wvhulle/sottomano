use std::{borrow::Cow, collections::BTreeMap, convert::Infallible, iter::once, time::Duration};

use chrono::{DateTime, Locale, Utc};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    ci_runner_tasks::{
        CiRunnerTasks, CiTask, RunnerAccess, TaskDeadline, TaskRepository, TaskUsage,
    },
    podman_container_engine::libpod_running_containers::EngineReadError,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        container_usage_text::usage_text,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiRunnerKind {
    Woodpecker,
    Forgejo,
}

impl CiRunnerKind {
    const fn family(self) -> WidgetGroupFamily {
        match self {
            CiRunnerKind::Woodpecker => WidgetGroupFamily::WoodpeckerTasks,
            CiRunnerKind::Forgejo => WidgetGroupFamily::ForgejoTasks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CiRunnerReading {
    Summary,
    Task { run_label: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiRunnerTarget {
    kind: CiRunnerKind,
    runner_label: String,
    reading: CiRunnerReading,
}

impl CiRunnerTarget {
    #[must_use]
    pub fn new(kind: CiRunnerKind, runner_label: &str, reading: CiRunnerReading) -> Self {
        Self {
            kind,
            runner_label: runner_label.to_owned(),
            reading,
        }
    }

    #[must_use]
    pub fn kind(&self) -> CiRunnerKind {
        self.kind
    }
}

impl From<CiRunnerTarget> for HoverTarget {
    fn from(target: CiRunnerTarget) -> Self {
        HoverTarget::CiRunner(target)
    }
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

struct CiRunnerSummaryRow<'a> {
    runner_label: &'a str,
    runner: &'a CiRunnerTasks,
}

impl GroupItemReading for CiRunnerSummaryRow<'_> {
    fn name(&self) -> &'static str {
        "CI runner"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.runner_label.to_owned().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::GitBranch(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        let running = format!("{} running", self.runner.tasks.len());
        Some(
            access_failure_text(&self.runner.access).map_or(running.clone(), |failure| {
                if self.runner.tasks.is_empty() {
                    failure.to_owned()
                } else {
                    format!("{running}, {failure}")
                }
            }),
        )
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.runner.access {
            RunnerAccess::Complete => ReadingDisplayState::Idle,
            RunnerAccess::EngineUnreadable(_) | RunnerAccess::AgentUnreachable(_) => {
                ReadingDisplayState::Failed
            }
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        match &self.runner.access {
            RunnerAccess::Complete => HoverCardBody::sections([Section::note(format!(
                "{} running tasks",
                self.runner.tasks.len()
            ))]),
            RunnerAccess::EngineUnreadable(failure) => {
                HoverCardBody::sections([Section::error(failure)])
            }
            RunnerAccess::AgentUnreachable(failure) => {
                HoverCardBody::sections([Section::error(failure)])
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

struct CiTaskRow<'a> {
    task: &'a CiTask,
}

impl GroupItemReading for CiTaskRow<'_> {
    fn name(&self) -> &'static str {
        "CI task"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        task_row_name(self.task).into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::GitCommit(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        let elapsed = duration_text(elapsed_of(self.task, clock), locale);
        Some(match self.task.usage {
            TaskUsage::NoContainerNow => elapsed,
            TaskUsage::Measured {
                cpu_load,
                resident_memory,
            } => format!("{elapsed}  {}", usage_text(cpu_load, resident_memory)),
        })
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
        HoverCardBody::sections([Section::dictionary(
            [
                LabeledValue::new("Task", task_full_name(self.task)),
                LabeledValue::new("Running", joined_or_none(&self.task.activities)),
                LabeledValue::new("Image", joined_or_none(&self.task.images)),
            ]
            .into_iter()
            .chain(deadline),
        )])
    }
}

pub trait CiRunnerSnapshot: SnapshotSource<Error = Infallible> {
    const KIND: CiRunnerKind;

    fn runner(&self) -> &CiRunnerTasks;
}

pub struct CiTasksGroup<S: CiRunnerSnapshot> {
    runners: BTreeMap<String, LatestSnapshot<S, HidesFetchFailures>>,
    expansion: WidgetGroupExpansion,
}

impl<S: CiRunnerSnapshot> Default for CiTasksGroup<S> {
    fn default() -> Self {
        Self {
            runners: BTreeMap::new(),
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CiTasksMessage<S: CiRunnerSnapshot> {
    Runner(String, SnapshotDelivery<S>),
    Expansion(WidgetGroupExpansionMessage),
}

impl<S: CiRunnerSnapshot> CiTasksGroup<S> {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, message: CiTasksMessage<S>, clock: Clock) {
        match message {
            CiTasksMessage::Runner(runner_label, delivery) => {
                self.runners
                    .entry(runner_label)
                    .or_default()
                    .receive(delivery, clock);
            }
            CiTasksMessage::Expansion(message) => {
                self.expansion.update(message);
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &CiRunnerTarget, clock: Clock) -> Option<HoverCard> {
        let reading = self.runners.get(&target.runner_label)?;
        let runner = reading.get()?.runner();
        let body = match &target.reading {
            CiRunnerReading::Summary => CiRunnerSummaryRow {
                runner_label: &target.runner_label,
                runner,
            }
            .hover_card(clock),
            CiRunnerReading::Task { run_label } => CiTaskRow {
                task: runner
                    .tasks
                    .iter()
                    .find(|task| &task.run_label == run_label)?,
            }
            .hover_card(clock),
        };
        reading.hover_card_with(body)
    }

    pub fn group_content<'a, M: Clone + 'a>(
        &'a self,
        runner_labels: impl IntoIterator<Item = &'a str>,
        lift_expansion: fn(WidgetGroupExpansionMessage) -> M,
    ) -> WidgetGroupContent<'a, M> {
        let items = runner_labels.into_iter().flat_map(|runner_label| {
            let runner = self
                .runners
                .get(runner_label)
                .and_then(LatestSnapshot::get)
                .map(CiRunnerSnapshot::runner);
            runner.into_iter().flat_map(move |runner| {
                let target = move |reading| CiRunnerTarget::new(S::KIND, runner_label, reading);
                once(GroupItem::reading(
                    CiRunnerSummaryRow {
                        runner_label,
                        runner,
                    },
                    target(CiRunnerReading::Summary),
                ))
                .chain(runner.tasks.iter().map(move |task| {
                    GroupItem::reading(
                        CiTaskRow { task },
                        target(CiRunnerReading::Task {
                            run_label: task.run_label.clone(),
                        }),
                    )
                }))
            })
        });
        WidgetGroupContent::expandable(S::KIND.family(), &self.expansion, lift_expansion, items)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use bytesize::ByteSize;
    use sottomano_desktop_sources::podman_container_engine::libpod_running_containers::ContainerCpuLoad;

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
                cpu_load: ContainerCpuLoad::BusyCores(1.0),
                resident_memory: ByteSize::mib(1),
            },
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
    fn a_denied_engine_is_explained_on_the_row() {
        assert_eq!(
            access_failure_text(&RunnerAccess::EngineUnreadable(
                EngineReadError::SocketDenied
            )),
            Some("no access: socket denied")
        );
        assert_eq!(access_failure_text(&RunnerAccess::Complete), None);
    }
}
