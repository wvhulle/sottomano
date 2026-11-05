use iced::Subscription;
use sottomano_desktop_sources::ci_runner_tasks::{
    CiRunnerTasks,
    forgejo_runner_tasks::{ForgejoRunnerTasks, ForgejoRunnerWatch},
};
use sottomano_toml_config::widget_groups::{
    ci_runners::CiRunnersConfig, podman_engines::resolved_runtime_path,
};

use crate::{
    background_work::snapshot_subscription,
    widget_groups::ci_task_group_item::{
        CiRunnerKind, CiRunnerSnapshot, CiTasksGroup, CiTasksMessage,
    },
};

impl CiRunnerSnapshot for ForgejoRunnerTasks {
    const KIND: CiRunnerKind = CiRunnerKind::Forgejo;

    fn runner(&self) -> &CiRunnerTasks {
        &self.runner
    }
}

pub type ForgejoTasksGroup = CiTasksGroup<ForgejoRunnerTasks>;
pub type ForgejoTasksMessage = CiTasksMessage<ForgejoRunnerTasks>;

pub fn forgejo_tasks_subscription(config: &CiRunnersConfig) -> Subscription<ForgejoTasksMessage> {
    Subscription::batch(config.forgejo_runners.iter().map(|runner| {
        snapshot_subscription::<ForgejoRunnerTasks>(ForgejoRunnerWatch {
            unit_name: runner.unit.clone(),
            engine_socket: resolved_runtime_path(&runner.engine_socket),
        })
        .with(runner.label.clone())
        .map(|(runner_label, delivery)| CiTasksMessage::Runner(runner_label, delivery))
    }))
}
