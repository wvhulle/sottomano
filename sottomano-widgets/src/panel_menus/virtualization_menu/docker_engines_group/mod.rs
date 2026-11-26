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
    podman_container_engine::libpod_running_containers::EngineListing,
};

use self::docker_engine_indicators::{ListedContainerIndicator, UnreadableEngineIndicator};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::{icon_glyph::IconGlyph, simple_icons::SimpleIcon},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
        indicator_state::IndicatorState,
        snapshot_absence::{AbsenceCause, awaiting_source_indicator},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    panel_menus::virtualization_menu::engine_container_indicators::{
        EngineContainerAddress, EngineContainerInteraction, EngineContainers,
        RunningContainerIndicator, StoppedContainerIndicator, container_switch_task,
        indicators_ending_in_hidden_count, with_container_switch,
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

    fn probe_availability(&self, engine: &DockerEngineSocket) -> Option<Availability> {
        self.probes
            .get(&engine.socket_path)?
            .get()
            .map(|probe| probe.engine_availability)
    }

    fn probe_presence_count(&self) -> Option<usize> {
        self.engines
            .iter()
            .map(|engine| match self.probe_availability(engine) {
                Some(Availability::Available) => Some(1),
                Some(Availability::Unavailable) | None => None,
            })
            .sum()
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
                docker_glyph(),
                self.shown_state(),
            ),
            &self.expansion,
            |expansion| {
                IndicatorGroupInteraction::DockerEngines(EngineContainerInteraction::Expansion(
                    expansion,
                ))
            },
            self.indicator_listing(),
        )
    }

    fn listed_indicators<'view>(
        engine: &'view DockerEngineSocket,
        engine_listings: &'view SubscribedSnapshot<DockerEngineListing>,
        listing: &'view DockerEngineListing,
    ) -> Vec<GroupIndicator<'view>> {
        listing
            .listings
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

    fn indicator_listing(&self) -> IndicatorListing<'_> {
        self.engines
            .iter()
            .map(|engine| self.engine_indicator_listing(engine))
            .collect::<IndicatorListing<'_>>()
            .with_listed(indicators_ending_in_hidden_count)
    }

    fn engine_indicator_listing<'view>(
        &'view self,
        engine: &'view DockerEngineSocket,
    ) -> IndicatorListing<'view> {
        match self.readouts.get(&engine.socket_path) {
            Some(
                engine_readout @ SubscribedSnapshot::Available {
                    snapshot: DockerEngineReadout { engine_listing },
                    ..
                },
            ) => IndicatorListing::Listed(
                Self::running_container_indicators(engine, engine_listing, engine_readout)
                    .into_iter()
                    .chain(Self::stopped_container_indicators(
                        engine,
                        engine_listing,
                        engine_readout,
                    ))
                    .collect(),
            ),
            Some(SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::FetchFailed(read_failure),
            }) => IndicatorListing::Incomplete {
                listed_indicators: Vec::new(),
                unread_sources: vec![
                    UnreadableEngineIndicator {
                        engine,
                        read_failure,
                    }
                    .indicator(),
                ],
            },
            Some(SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::SourceAbsent,
            }) => IndicatorListing::Listed(Vec::new()),
            Some(SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::Awaiting,
            })
            | None => match self.listings.get(&engine.socket_path) {
                _ if self.probe_availability(engine) == Some(Availability::Unavailable) => {
                    IndicatorListing::Listed(Vec::new())
                }
                Some(engine_listings) => engine_listings.indicator_listing(
                    |listing| Self::listed_indicators(engine, engine_listings, listing),
                    docker_glyph(),
                ),
                None => IndicatorListing::Incomplete {
                    listed_indicators: Vec::new(),
                    unread_sources: vec![awaiting_source_indicator::<DockerEngineListing>(
                        docker_glyph(),
                    )],
                },
            },
        }
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

fn docker_glyph() -> IconGlyph {
    IconGlyph::SimpleIcon(SimpleIcon::Docker)
}
