use iced::Subscription;
use sottomano_desktop_sources::ci_runner_tasks::{
    CiRunnerTasks,
    woodpecker_agent_varz::{WoodpeckerAgentTasks, WoodpeckerAgentWatch},
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

impl CiRunnerSnapshot for WoodpeckerAgentTasks {
    const KIND: CiRunnerKind = CiRunnerKind::Woodpecker;

    fn runner(&self) -> &CiRunnerTasks {
        &self.runner
    }
}

pub type WoodpeckerTasksGroup = CiTasksGroup<WoodpeckerAgentTasks>;
pub type WoodpeckerTasksMessage = CiTasksMessage<WoodpeckerAgentTasks>;

pub fn woodpecker_tasks_subscription(
    config: &CiRunnersConfig,
) -> Subscription<WoodpeckerTasksMessage> {
    Subscription::batch(config.woodpecker_agents.iter().map(|agent| {
        snapshot_subscription::<WoodpeckerAgentTasks>(WoodpeckerAgentWatch {
            varz_url: agent.varz_url.clone(),
            engine_socket: resolved_runtime_path(&agent.engine_socket),
        })
        .with(agent.label.clone())
        .map(|(runner_label, delivery)| CiTasksMessage::Runner(runner_label, delivery))
    }))
}
