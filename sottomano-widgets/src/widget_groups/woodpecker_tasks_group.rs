use sottomano_desktop_sources::{
    ci_runner_tasks::{
        CiRunnerTasks,
        systemd_service_runner_discovery::DiscoveredCiRunner,
        woodpecker_agent_tasks::{WoodpeckerAgentTasks, WoodpeckerAgentWatch},
    },
    systemd_units_journal::{SystemdUnitScope, service_commands::ServiceCommand},
};

use crate::widget_groups::ci_task_group_item::{
    CiRunnerKind, CiRunnerSnapshot, CiTasksGroup, CiTasksMessage,
};

impl CiRunnerSnapshot for WoodpeckerAgentTasks {
    const KIND: CiRunnerKind = CiRunnerKind::Woodpecker;

    fn runner(&self) -> &CiRunnerTasks {
        &self.runner
    }

    fn discovered(
        scope: SystemdUnitScope,
        command: &ServiceCommand,
    ) -> Option<DiscoveredCiRunner<WoodpeckerAgentWatch>> {
        WoodpeckerAgentWatch::discovered(scope, command)
    }
}

pub type WoodpeckerTasksGroup = CiTasksGroup<WoodpeckerAgentTasks>;
pub type WoodpeckerTasksMessage = CiTasksMessage<WoodpeckerAgentTasks>;
