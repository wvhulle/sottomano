use sottomano_desktop_sources::{
    ci_runner_tasks::{
        CiRunnerTasks,
        forgejo_runner_tasks::{ForgejoRunnerTasks, ForgejoRunnerWatch},
        systemd_service_runner_discovery::DiscoveredCiRunner,
    },
    systemd_units_journal::{SystemdUnitScope, service_commands::ServiceCommand},
};

use crate::widget_groups::ci_task_group_item::{
    CiRunnerKind, CiRunnerSnapshot, CiTasksGroup, CiTasksMessage,
};

impl CiRunnerSnapshot for ForgejoRunnerTasks {
    const KIND: CiRunnerKind = CiRunnerKind::Forgejo;

    fn runner(&self) -> &CiRunnerTasks {
        &self.runner
    }

    fn discovered(
        scope: SystemdUnitScope,
        command: &ServiceCommand,
    ) -> Option<DiscoveredCiRunner<ForgejoRunnerWatch>> {
        ForgejoRunnerWatch::discovered(scope, command)
    }
}

pub type ForgejoTasksGroup = CiTasksGroup<ForgejoRunnerTasks>;
pub type ForgejoTasksMessage = CiTasksMessage<ForgejoRunnerTasks>;
