mod docker_engine_indicators;
#[cfg(test)]
mod docker_engines_group_tests;

use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    container_power::ContainerEngineApi, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    docker_container_engine::{
        DockerEngineListing, DockerEngineProbe, DockerEngineReadout, DockerEngineSocket,
        candidate_docker_sockets,
    },
    podman_container_engine::libpod_running_containers::{EngineListing, EngineReadError},
};

use self::docker_engine_indicators::{ListedContainerIndicator, UnreadableEngineIndicator};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::{icon_glyph::IconGlyph, simple_icons::SimpleIcon},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        engine_container_indicators::{
            EngineContainerAddress, EngineContainerInteraction, EngineContainers,
            RunningContainerIndicator, StoppedContainerIndicator, container_switch_task,
            indicators_ending_in_hidden_count, with_container_switch,
        },
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub struct DockerEnginesGroup {
    engines: Vec<DockerEngineSocket>,
    probes: BTreeMap<PathBuf, SubscribedSnapshot<DockerEngineProbe>>,
    listings: BTreeMap<PathBuf, SubscribedSnapshot<DockerEngineListing>>,
    readouts: BTreeMap<PathBuf, SubscribedSnapshot<DockerEngineReadout>>,
    expansion: IndicatorGroupExpansion,
}

impl Default for DockerEnginesGroup {
    fn default() -> Self {
        Self::watching(candidate_docker_sockets())
    }
}

#[derive(Debug)]
pub enum DockerEnginesMessage {
    Probe(PathBuf, SnapshotDelivery<DockerEngineProbe>),
    Listing(PathBuf, SnapshotDelivery<DockerEngineListing>),
    Engine(PathBuf, SnapshotDelivery<DockerEngineReadout>),
    PowerChangeRefused(GuestPowerError),
}

pub type DockerEnginesInteraction = EngineContainerInteraction;

impl DockerEnginesGroup {
    #[must_use]
    pub fn watching(engines: Vec<DockerEngineSocket>) -> Self {
        Self {
            engines,
            probes: BTreeMap::new(),
            listings: BTreeMap::new(),
            readouts: BTreeMap::new(),
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<DockerEnginesMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        Subscription::batch(self.engines.iter().flat_map(|engine| {
            let socket_path = engine.socket_path.clone();
            [
                snapshot_subscription::<DockerEngineProbe>(socket_path.clone())
                    .with(socket_path.clone())
                    .map(|(socket_path, delivery)| {
                        DockerEnginesMessage::Probe(socket_path, delivery)
                    }),
                demand
                    .while_headline_shown(
                        snapshot_subscription::<DockerEngineListing>(socket_path.clone())
                            .with(socket_path.clone()),
                    )
                    .map(|(socket_path, delivery)| {
                        DockerEnginesMessage::Listing(socket_path, delivery)
                    }),
                demand
                    .while_indicators_shown(
                        snapshot_subscription::<DockerEngineReadout>(socket_path.clone())
                            .with(socket_path),
                    )
                    .map(|(socket_path, delivery)| {
                        DockerEnginesMessage::Engine(socket_path, delivery)
                    }),
            ]
        }))
    }

    pub fn update(
        &mut self,
        message: DockerEnginesMessage,
        clock: Clock,
    ) -> Task<DockerEnginesMessage> {
        match message {
            DockerEnginesMessage::Probe(socket_path, delivery) => {
                self.probes
                    .entry(socket_path)
                    .or_default()
                    .receive(delivery, clock);
            }
            DockerEnginesMessage::Listing(socket_path, delivery) => {
                self.listings
                    .entry(socket_path)
                    .or_default()
                    .receive(delivery, clock);
            }
            DockerEnginesMessage::Engine(socket_path, delivery) => {
                self.readouts
                    .entry(socket_path)
                    .or_default()
                    .receive(delivery, clock);
            }
            DockerEnginesMessage::PowerChangeRefused(refusal) => {
                tracing::warn!(%refusal, "docker container power change refused");
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: DockerEnginesInteraction,
    ) -> Task<DockerEnginesMessage> {
        match interaction {
            EngineContainerInteraction::SwitchContainer(request) => container_switch_task(
                ContainerEngineApi::Docker,
                request,
                DockerEnginesMessage::PowerChangeRefused,
            ),
            EngineContainerInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn probe_presence_count(&self) -> Option<usize> {
        let mut presence_count = 0;
        for engine in &self.engines {
            let present = self
                .probes
                .get(&engine.socket_path)
                .and_then(|probe| probe.get())
                .is_some_and(|probe| probe.engine_availability == Availability::Available);
            if !present {
                return None;
            }
            presence_count += 1;
        }
        Some(presence_count)
    }

    fn engine_readout_listing(&self, engine: &DockerEngineSocket) -> Option<&EngineListing> {
        self.readouts.get(&engine.socket_path)?.get().map(
            |DockerEngineReadout {
                 engine_listing: listing,
             }| listing,
        )
    }

    fn readout_failure(&self, engine: &DockerEngineSocket) -> Option<&EngineReadError> {
        self.readouts.get(&engine.socket_path)?.fetch_failure()
    }

    fn engine_container_listing(
        &self,
        engine: &DockerEngineSocket,
    ) -> Option<&DockerEngineListing> {
        self.listings.get(&engine.socket_path)?.get()
    }

    fn running_container_indicators<'view>(
        engine: &'view DockerEngineSocket,
        listing: &'view EngineListing,
        engine_readout: &'view SubscribedSnapshot<DockerEngineReadout>,
    ) -> Vec<GroupIndicator<'view>> {
        let engine_containers = EngineContainers {
            socket_path: &engine.socket_path,
            owner: &engine.owner,
            listing,
        };
        engine_containers
            .running_indicators(|request| {
                IndicatorGroupInteraction::DockerEngines(
                    EngineContainerInteraction::SwitchContainer(request),
                )
            })
            .into_iter()
            .zip(&listing.running.containers)
            .map(|(row, container)| {
                row.with_hover_card(Box::new(move |clock| {
                    engine_readout.hover_card_with(
                        RunningContainerIndicator {
                            container,
                            owner: &engine.owner,
                        }
                        .hover_card(clock),
                    )
                }))
            })
            .collect()
    }

    fn stopped_container_indicators<'view>(
        engine: &'view DockerEngineSocket,
        listing: &'view EngineListing,
        engine_readout: &'view SubscribedSnapshot<DockerEngineReadout>,
    ) -> Vec<GroupIndicator<'view>> {
        let engine_containers = EngineContainers {
            socket_path: &engine.socket_path,
            owner: &engine.owner,
            listing,
        };
        engine_containers
            .stopped_indicators(|request| {
                IndicatorGroupInteraction::DockerEngines(
                    EngineContainerInteraction::SwitchContainer(request),
                )
            })
            .into_iter()
            .zip(&listing.stopped)
            .map(|(row, container)| {
                row.with_hover_card(Box::new(move |_| {
                    engine_readout.hover_card_with(
                        StoppedContainerIndicator {
                            container,
                            owner: &engine.owner,
                        }
                        .hover_card(),
                    )
                }))
            })
            .collect()
    }

    #[must_use]
    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::DockerEngines,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::DockerEngines.title(),
                IconGlyph::SimpleIcon(SimpleIcon::Docker),
                self.shown_state(),
            ),
            &self.expansion,
            |expansion| {
                IndicatorGroupInteraction::DockerEngines(EngineContainerInteraction::Expansion(
                    expansion,
                ))
            },
            self.container_indicators(),
        )
    }

    fn listed_indicators<'view>(
        &'view self,
        engine: &'view DockerEngineSocket,
    ) -> Vec<GroupIndicator<'view>> {
        let Some(engine_listings) = self.listings.get(&engine.socket_path) else {
            return Vec::new();
        };
        self.engine_container_listing(engine)
            .map(|listing| listing.listings.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|listed| {
                let target = EngineContainerAddress {
                    socket_path: Cow::Owned(engine.socket_path.display().to_string()),
                    container_id: Cow::Owned(listed.id.clone()),
                };
                with_container_switch(
                    ListedContainerIndicator { engine, listed }.indicator(),
                    listed.run_state,
                    target,
                    |request| {
                        IndicatorGroupInteraction::DockerEngines(
                            EngineContainerInteraction::SwitchContainer(request),
                        )
                    },
                )
                .with_hover_card(Box::new(move |_| {
                    engine_listings
                        .hover_card_with(ListedContainerIndicator { engine, listed }.hover_card())
                }))
            })
            .collect()
    }

    fn container_indicators(&self) -> Vec<GroupIndicator<'_>> {
        let mut failure_indicators = Vec::new();
        let mut running_indicators = Vec::new();
        let mut stopped_indicators = Vec::new();
        for engine in &self.engines {
            if let Some(engine_readout) = self.readouts.get(&engine.socket_path)
                && let Some(listing) = self.engine_readout_listing(engine)
            {
                running_indicators.extend(Self::running_container_indicators(
                    engine,
                    listing,
                    engine_readout,
                ));
                stopped_indicators.extend(Self::stopped_container_indicators(
                    engine,
                    listing,
                    engine_readout,
                ));
            } else if let Some(read_failure) = self.readout_failure(engine) {
                failure_indicators.push(
                    UnreadableEngineIndicator {
                        engine,
                        read_failure,
                    }
                    .indicator(),
                );
            } else {
                running_indicators.extend(self.listed_indicators(engine));
            }
        }
        running_indicators.extend(stopped_indicators);
        failure_indicators.extend(indicators_ending_in_hidden_count(running_indicators));
        failure_indicators
    }
}

impl DockerEnginesGroup {
    fn shown_state(&self) -> IndicatorState {
        let any_engine_present = self
            .probe_presence_count()
            .is_some_and(|presence_count| presence_count > 0);
        if any_engine_present {
            IndicatorState::On
        } else {
            IndicatorState::Off
        }
    }
}
