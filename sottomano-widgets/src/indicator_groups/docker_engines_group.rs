use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    container_power::ContainerEngineApi, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    docker_container_engine::{
        DockerContainerListing, DockerEngineListing, DockerEngineProbe, DockerEngineReadout,
        DockerEngineSocket, candidate_docker_sockets,
    },
    podman_container_engine::libpod_running_containers::{EngineListing, EngineReadError},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    indicator_groups::{
        engine_container_indicators::{
            EngineContainerAddress, EngineContainerInteraction, EngineContainers,
            RunningContainerIndicator, StoppedContainerIndicator, container_switch_task,
            engine_container_name, engine_failure_label, indicators_ending_in_hidden_count,
            with_container_switch,
        },
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

struct UnreadableEngineIndicator<'a> {
    engine: &'a DockerEngineSocket,
    read_failure: &'a EngineReadError,
}

impl<'a> UnreadableEngineIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            self.engine.owner.to_string(),
            Phosphor::Package(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Error",
                SectionWidget::paragraph(self.read_failure.to_string()),
            ),
            Section::new(
                "Connection",
                dictionary_table([LabeledValue::new(
                    "Socket",
                    self.engine.socket_path.display().to_string(),
                )]),
            ),
        ]
    }
}

struct ListedContainerIndicator<'a> {
    engine: &'a DockerEngineSocket,
    listed: &'a DockerContainerListing,
}

impl<'a> ListedContainerIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            engine_container_name(&self.listed.container_name, &self.engine.owner),
            Phosphor::Package(Weight::Regular).into(),
            self.listed.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.listed.run_state.label().to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("State", self.listed.run_state.label()),
                LabeledValue::new("Engine", self.engine.owner.to_string()),
            ]),
        )]
    }
}

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
                .is_some_and(|probe| probe.0 == Availability::Available);
            if !present {
                return None;
            }
            presence_count += 1;
        }
        Some(presence_count)
    }

    fn readout_of(&self, engine: &DockerEngineSocket) -> Option<&EngineListing> {
        self.readouts
            .get(&engine.socket_path)?
            .get()
            .map(|DockerEngineReadout(listing)| listing)
    }

    fn readout_failure(&self, engine: &DockerEngineSocket) -> Option<&EngineReadError> {
        self.readouts.get(&engine.socket_path)?.fetch_failure()
    }

    fn listing_of(&self, engine: &DockerEngineSocket) -> Option<&DockerEngineListing> {
        self.listings.get(&engine.socket_path)?.get()
    }

    fn running_container_indicators<'a>(
        engine: &'a DockerEngineSocket,
        listing: &'a EngineListing,
        engine_readout: &'a SubscribedSnapshot<DockerEngineReadout>,
    ) -> Vec<GroupIndicator<'a>> {
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

    fn stopped_container_indicators<'a>(
        engine: &'a DockerEngineSocket,
        listing: &'a EngineListing,
        engine_readout: &'a SubscribedSnapshot<DockerEngineReadout>,
    ) -> Vec<GroupIndicator<'a>> {
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

    fn listed_indicators<'a>(&'a self, engine: &'a DockerEngineSocket) -> Vec<GroupIndicator<'a>> {
        let Some(engine_listings) = self.listings.get(&engine.socket_path) else {
            return Vec::new();
        };
        self.listing_of(engine)
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
                && let Some(listing) = self.readout_of(engine)
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        podman_container_engine::{
            libpod_running_containers::{
                EngineContainers, EngineListing, EngineReadError, StoppedContainer,
            },
            systemd_service_engine_discovery::EngineOwner,
        },
    };

    use super::*;
    use crate::{
        hover_cards::{HoverTarget, IndicatorKey},
        indicator_groups::indicator_group_identity::IndicatorGroupIdentity,
    };

    fn delivered(
        engine_read: Result<EngineListing, EngineReadError>,
    ) -> SnapshotDelivery<DockerEngineReadout> {
        SnapshotDelivery::fixture_delivered(
            engine_read.map(DockerEngineReadout),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )
    }

    const ROOTFUL_SOCKET_PATH: &str = "/run/docker.sock";

    fn rootful_engine_group() -> DockerEnginesGroup {
        DockerEnginesGroup::watching(vec![DockerEngineSocket {
            owner: EngineOwner::Root,
            socket_path: PathBuf::from(ROOTFUL_SOCKET_PATH),
        }])
    }

    #[test]
    fn a_denied_docker_socket_shows_a_failure_indicator() {
        let rootful_socket = PathBuf::from(ROOTFUL_SOCKET_PATH);
        let mut group = rootful_engine_group();
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket.clone(),
                delivered(Err(EngineReadError::SocketDenied)),
            ),
            Clock::real(),
        );
        assert_eq!(group.container_indicators().len(), 1);
        let failure_indicator = HoverTarget::Indicator(IndicatorKey {
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::DockerEngines),
            name: group.container_indicators()[0].indicator_name(),
        });
        assert!(
            group
                .group_content()
                .hover_card(&failure_indicator, Clock::real())
                .is_some_and(|card| !card.sections.is_empty())
        );
    }

    #[test]
    fn a_missing_docker_socket_shows_no_indicator() {
        let rootful_socket = PathBuf::from(ROOTFUL_SOCKET_PATH);
        let mut group = rootful_engine_group();
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket.clone(),
                SnapshotDelivery::SourceUnavailable,
            ),
            Clock::real(),
        );
        assert!(group.group_content().is_empty());
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket,
                delivered(Ok(EngineListing {
                    running: EngineContainers {
                        containers: Vec::new(),
                    },
                    stopped: vec![StoppedContainer {
                        id: "exited".to_owned(),
                        container_name: "exited".to_owned(),
                        shown_name: "exited".to_owned(),
                        image: "alpine".to_owned(),
                        stopped_at: UNIX_EPOCH,
                    }],
                })),
            ),
            Clock::real(),
        );
        assert!(!group.group_content().is_empty());
    }
}
