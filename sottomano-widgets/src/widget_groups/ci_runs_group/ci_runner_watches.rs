use std::{collections::BTreeMap, convert::Infallible};

use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    ci_runner_tasks::{
        CiRunnerTasks,
        forgejo_runner_tasks::{ForgejoRunnerTasks, ForgejoRunnerWatch},
        systemd_service_runner_discovery::DiscoveredCiRunner,
        woodpecker_agent_tasks::{WoodpeckerAgentTasks, WoodpeckerAgentWatch},
    },
    systemd_units_journal::{SystemdUnitScope, service_commands::ServiceCommand},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    widget_groups::ci_runs_group::ci_run_readings::CiRunnerKind,
    widgets::{
        discovered_systemd_services::DiscoveredSystemdServices,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
    },
};

pub trait CiRunnerSnapshot: SnapshotSource<Error = Infallible> {
    const KIND: CiRunnerKind;

    fn runner(&self) -> &CiRunnerTasks;

    fn discovered(
        scope: SystemdUnitScope,
        command: &ServiceCommand,
    ) -> Option<DiscoveredCiRunner<Self::StreamInputs>>;
}

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

pub struct CiRunnerWatches<S: CiRunnerSnapshot> {
    discovered_runners: Vec<DiscoveredCiRunner<S::StreamInputs>>,
    runners: BTreeMap<String, LatestSnapshot<S, HidesFetchFailures>>,
}

impl<S: CiRunnerSnapshot> Default for CiRunnerWatches<S> {
    fn default() -> Self {
        Self {
            discovered_runners: Vec::new(),
            runners: BTreeMap::new(),
        }
    }
}

impl<S: CiRunnerSnapshot> CiRunnerWatches<S> {
    pub fn subscription(&self) -> Subscription<(String, SnapshotDelivery<S>)> {
        Subscription::batch(self.discovered_runners.iter().map(|discovered| {
            snapshot_subscription::<S>(discovered.watch.clone()).with(discovered.label.clone())
        }))
    }

    pub fn adopt_discovered(&mut self, services: &DiscoveredSystemdServices) {
        let discovered_runners: Vec<DiscoveredCiRunner<S::StreamInputs>> =
            services.discovered(S::discovered).collect();
        self.runners.retain(|runner_label, _| {
            discovered_runners
                .iter()
                .any(|discovered| &discovered.label == runner_label)
        });
        self.discovered_runners = discovered_runners;
    }

    pub fn receive(&mut self, runner_label: String, delivery: SnapshotDelivery<S>, clock: Clock) {
        self.runners
            .entry(runner_label)
            .or_default()
            .receive(delivery, clock);
    }

    #[must_use]
    pub fn snapshot_of(
        &self,
        runner_label: &str,
    ) -> Option<&LatestSnapshot<S, HidesFetchFailures>> {
        self.runners.get(runner_label)
    }

    pub fn delivered_runners(&self) -> impl Iterator<Item = (&str, &CiRunnerTasks)> {
        self.discovered_runners.iter().filter_map(|discovered| {
            let runner = self
                .runners
                .get(&discovered.label)
                .and_then(LatestSnapshot::get)?
                .runner();
            Some((discovered.label.as_str(), runner))
        })
    }
}
