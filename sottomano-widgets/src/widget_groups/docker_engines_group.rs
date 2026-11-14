use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::container_power::ContainerEngineApi;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    docker_container_engine::{
        DockerContainerListing, DockerEngineListing, DockerEngineProbe, DockerEngineReadout,
        DockerEngineSocket, candidate_docker_sockets,
    },
    podman_container_engine::libpod_running_containers::{EngineReadError, EngineReadout},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        engine_container_rows::{
            EngineContainerInteraction, EngineContainerTarget, EngineContainers,
            container_switch_task, engine_container_name, rows_ending_in_hidden_count,
            with_container_switch,
        },
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion, WidgetGroupFace},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        simple_icons::SimpleIcon,
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerEngineTarget {
    Engine { socket_path: Cow<'static, str> },
    Container(EngineContainerTarget),
    HiddenContainers,
}

impl DockerEngineTarget {
    #[must_use]
    pub const fn of_socket(socket_path: Cow<'static, str>) -> Self {
        Self::Engine { socket_path }
    }

    fn of_engine(engine: &DockerEngineSocket) -> Self {
        Self::of_socket(Cow::Owned(engine.socket_path.display().to_string()))
    }
}

impl From<DockerEngineTarget> for HoverTarget {
    fn from(target: DockerEngineTarget) -> Self {
        HoverTarget::DockerEngine(target)
    }
}

fn container_hover_target(target: EngineContainerTarget) -> HoverTarget {
    DockerEngineTarget::Container(target).into()
}

struct UnreadableEngineRow<'a> {
    engine: &'a DockerEngineSocket,
    failure: &'a EngineReadError,
}

impl GroupItemReading for UnreadableEngineRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        self.engine.owner.to_string().into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Docker)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(match self.failure {
            EngineReadError::SocketDenied => "no access: socket denied".to_owned(),
            _ => "unreadable".to_owned(),
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([
            Section::error(self.failure),
            Section::dictionary([LabeledValue::new(
                "Socket",
                self.engine.socket_path.display().to_string(),
            )]),
        ])
    }
}

struct ListedContainerRow<'a> {
    engine: &'a DockerEngineSocket,
    listed: &'a DockerContainerListing,
}

impl GroupItemReading for ListedContainerRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        engine_container_name(&self.listed.container_name, &self.engine.owner)
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Docker)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.listed.run_state.label().to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.listed.run_state.into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("State", self.listed.run_state.label()),
            LabeledValue::new("Engine", self.engine.owner.to_string()),
        ])
    }
}

pub struct DockerEnginesGroup {
    engines: Vec<DockerEngineSocket>,
    probes: BTreeMap<PathBuf, LatestSnapshot<DockerEngineProbe, HidesFetchFailures>>,
    listings: BTreeMap<PathBuf, LatestSnapshot<DockerEngineListing, HidesFetchFailures>>,
    readouts: BTreeMap<PathBuf, LatestSnapshot<DockerEngineReadout, HidesFetchFailures>>,
    expansion: WidgetGroupExpansion,
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
    PowerChangeRefused(String),
    Interaction(DockerEnginesInteraction),
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
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<DockerEnginesMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch(self.engines.iter().flat_map(|engine| {
            let socket_path = engine.socket_path.clone();
            [
                snapshot_subscription::<DockerEngineProbe>(socket_path.clone())
                    .with(socket_path.clone())
                    .map(|(socket_path, delivery)| {
                        DockerEnginesMessage::Probe(socket_path, delivery)
                    }),
                demand
                    .while_face_shown(
                        snapshot_subscription::<DockerEngineListing>(socket_path.clone())
                            .with(socket_path.clone()),
                    )
                    .map(|(socket_path, delivery)| {
                        DockerEnginesMessage::Listing(socket_path, delivery)
                    }),
                demand
                    .while_rows_shown(
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
            DockerEnginesMessage::Interaction(EngineContainerInteraction::SwitchContainer(
                request,
            )) => {
                return container_switch_task(
                    ContainerEngineApi::Docker,
                    request,
                    DockerEnginesMessage::PowerChangeRefused,
                );
            }
            DockerEnginesMessage::Interaction(EngineContainerInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
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

    fn readout_of(&self, engine: &DockerEngineSocket) -> Option<&EngineReadout> {
        self.readouts
            .get(&engine.socket_path)?
            .get()
            .map(|DockerEngineReadout(readout)| readout)
    }

    fn listing_of(&self, engine: &DockerEngineSocket) -> Option<&DockerEngineListing> {
        self.listings.get(&engine.socket_path)?.get()
    }

    fn engine_named(&self, socket_path: &Path) -> Option<&DockerEngineSocket> {
        self.engines
            .iter()
            .find(|engine| engine.socket_path == socket_path)
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &DockerEngineTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            DockerEngineTarget::Engine { socket_path } => {
                let engine = self.engine_named(Path::new(socket_path.as_ref()))?;
                let EngineReadout::Unreadable(failure) = self.readout_of(engine)? else {
                    return None;
                };
                self.readouts
                    .get(&engine.socket_path)?
                    .hover_card_with(UnreadableEngineRow { engine, failure }.hover_card(clock))
            }
            DockerEngineTarget::Container(container_target) => {
                let engine = self
                    .engines
                    .iter()
                    .find(|engine| container_target.names_socket(&engine.socket_path))?;
                if let Some(EngineReadout::Containers(listing)) = self.readout_of(engine) {
                    let engine_containers = EngineContainers {
                        socket_path: &engine.socket_path,
                        owner: &engine.owner,
                        listing,
                    };
                    return self.readouts.get(&engine.socket_path)?.hover_card_with(
                        engine_containers.hover_body_of(&container_target.container_id, clock)?,
                    );
                }
                let listed = self
                    .listing_of(engine)?
                    .listings
                    .iter()
                    .find(|listed| listed.id == container_target.container_id.as_ref())?;
                self.listings
                    .get(&engine.socket_path)?
                    .hover_card_with(ListedContainerRow { engine, listed }.hover_card(clock))
            }
            DockerEngineTarget::HiddenContainers => None,
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, DockerEnginesInteraction> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::DockerEngines,
            &self.expansion,
            EngineContainerInteraction::Expansion,
            self.container_rows(),
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: IconGlyph::SimpleIcon(SimpleIcon::Docker),
            reading: self,
        })
    }

    fn listed_rows<'a>(
        &'a self,
        engine: &'a DockerEngineSocket,
    ) -> Vec<GroupItem<'a, DockerEnginesInteraction>> {
        self.listing_of(engine)
            .map(|listing| listing.listings.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|listed| {
                let target = EngineContainerTarget {
                    socket_path: Cow::Owned(engine.socket_path.display().to_string()),
                    container_id: Cow::Owned(listed.id.clone()),
                };
                with_container_switch(
                    GroupItem::reading(
                        ListedContainerRow { engine, listed },
                        container_hover_target(target.clone()),
                    ),
                    listed.run_state,
                    target,
                    EngineContainerInteraction::SwitchContainer,
                )
            })
            .collect()
    }

    fn container_rows(&self) -> Vec<GroupItem<'_, DockerEnginesInteraction>> {
        let mut failure_rows = Vec::new();
        let mut running_rows = Vec::new();
        let mut stopped_rows = Vec::new();
        for engine in &self.engines {
            match self.readout_of(engine) {
                Some(EngineReadout::Containers(listing)) => {
                    let engine_containers = EngineContainers {
                        socket_path: &engine.socket_path,
                        owner: &engine.owner,
                        listing,
                    };
                    running_rows.extend(engine_containers.running_rows(
                        container_hover_target,
                        EngineContainerInteraction::SwitchContainer,
                    ));
                    stopped_rows.extend(engine_containers.stopped_rows(
                        container_hover_target,
                        EngineContainerInteraction::SwitchContainer,
                    ));
                }
                Some(EngineReadout::Unreadable(EngineReadError::SocketMissing)) => {}
                Some(EngineReadout::Unreadable(failure)) => failure_rows.push(GroupItem::reading(
                    UnreadableEngineRow { engine, failure },
                    DockerEngineTarget::of_engine(engine),
                )),
                None => running_rows.extend(self.listed_rows(engine)),
            }
        }
        running_rows.extend(stopped_rows);
        failure_rows.extend(rows_ending_in_hidden_count(
            running_rows,
            DockerEngineTarget::HiddenContainers.into(),
        ));
        failure_rows
    }
}

impl GroupItemReading for DockerEnginesGroup {
    fn name(&self) -> Cow<'static, str> {
        "Docker engines".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Docker)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe_presence_count().map(|presence_count| {
            format!("{presence_count} of {} engines present", self.engines.len())
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        let any_engine_present = self
            .probe_presence_count()
            .is_some_and(|presence_count| presence_count > 0);
        if any_engine_present {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            self.probe_presence_count()
                .map(|presence_count| {
                    vec![
                        LabeledValue::new("Present", presence_count.to_string()),
                        LabeledValue::new("Engines", self.engines.len().to_string()),
                    ]
                })
                .unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        podman_container_engine::{
            libpod_running_containers::{EngineContainers, EngineListing, StoppedContainer},
            systemd_service_engine_discovery::EngineOwner,
        },
    };

    use super::*;

    fn delivered(readout: EngineReadout) -> SnapshotDelivery<DockerEngineReadout> {
        SnapshotDelivery::fixture_delivered(
            Ok(DockerEngineReadout(readout)),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )
    }

    #[test]
    fn a_missing_docker_socket_shows_no_row() {
        let rootful_socket = PathBuf::from("/run/docker.sock");
        let mut group = DockerEnginesGroup::watching(vec![DockerEngineSocket {
            owner: EngineOwner::Root,
            socket_path: rootful_socket.clone(),
        }]);
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket.clone(),
                delivered(EngineReadout::Unreadable(EngineReadError::SocketMissing)),
            ),
            Clock::real(),
        );
        assert!(group.group_content().is_empty());
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket,
                delivered(EngineReadout::Containers(EngineListing {
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
