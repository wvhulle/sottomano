use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use chrono::Locale;
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
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState, reading_row::ReadingRow,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, SectionWidget, dictionary_table},
    },
    widget_groups::{
        engine_container_rows::{
            EngineContainerInteraction, EngineContainerTarget, EngineContainers,
            container_switch_task, engine_container_name, engine_failure_label,
            rows_ending_in_hidden_count, with_container_switch,
        },
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion, WidgetGroupFace},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph, simple_icons::SimpleIcon, subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockerEngineTarget {
    Engine { socket_path: Cow<'static, str> },
    Container(EngineContainerTarget),
    HiddenContainers,
}

impl From<&DockerEngineSocket> for DockerEngineTarget {
    fn from(engine: &DockerEngineSocket) -> Self {
        Self::Engine {
            socket_path: Cow::Owned(engine.socket_path.display().to_string()),
        }
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
    read_failure: &'a EngineReadError,
}

impl GroupItemReading for UnreadableEngineRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        self.engine.owner.to_string().into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Docker)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(engine_failure_label(self.read_failure).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
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

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
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
    PowerChangeRefused(GuestPowerError),
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

    fn engine_named(&self, socket_path: &Path) -> Option<&DockerEngineSocket> {
        self.engines
            .iter()
            .find(|engine| engine.socket_path == socket_path)
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &DockerEngineTarget, clock: Clock) -> Vec<Section> {
        match target {
            DockerEngineTarget::Engine { socket_path } => {
                let Some(engine) = self.engine_named(Path::new(socket_path.as_ref())) else {
                    return Vec::new();
                };
                let Some(read_failure) = self.readout_failure(engine) else {
                    return Vec::new();
                };
                UnreadableEngineRow {
                    engine,
                    read_failure,
                }
                .hover_card(clock)
            }
            DockerEngineTarget::Container(container_target) => {
                let Some(engine) = self
                    .engines
                    .iter()
                    .find(|engine| container_target.names_socket(&engine.socket_path))
                else {
                    return Vec::new();
                };
                if let Some(listing) = self.readout_of(engine) {
                    let engine_containers = EngineContainers {
                        socket_path: &engine.socket_path,
                        owner: &engine.owner,
                        listing,
                    };
                    let Some(engine_readout) = self.readouts.get(&engine.socket_path) else {
                        return Vec::new();
                    };
                    let Some(container_body) =
                        engine_containers.hover_body_of(&container_target.container_id, clock)
                    else {
                        return Vec::new();
                    };
                    return engine_readout.hover_card_with(container_body);
                }
                let Some(listed) = self.listing_of(engine).and_then(|engine_listing| {
                    engine_listing
                        .listings
                        .iter()
                        .find(|listed| listed.id == container_target.container_id.as_ref())
                }) else {
                    return Vec::new();
                };
                let Some(engine_listings) = self.listings.get(&engine.socket_path) else {
                    return Vec::new();
                };
                engine_listings
                    .hover_card_with(ListedContainerRow { engine, listed }.hover_card(clock))
            }
            DockerEngineTarget::HiddenContainers => Vec::new(),
        }
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, DockerEnginesInteraction> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::DockerEngines,
            &self.expansion,
            EngineContainerInteraction::Expansion,
            self.container_rows(clock, locale),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: IconGlyph::SimpleIcon(SimpleIcon::Docker),
            state: self.state(clock),
        })
    }

    fn listed_rows<'a>(
        &'a self,
        engine: &'a DockerEngineSocket,
        clock: Clock,
        locale: Locale,
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
                        ReadingRow::evaluated(ListedContainerRow { engine, listed }, clock, locale),
                        container_hover_target(target.clone()),
                    ),
                    listed.run_state,
                    target,
                    EngineContainerInteraction::SwitchContainer,
                )
            })
            .collect()
    }

    fn container_rows(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Vec<GroupItem<'_, DockerEnginesInteraction>> {
        let mut failure_rows = Vec::new();
        let mut running_rows = Vec::new();
        let mut stopped_rows = Vec::new();
        for engine in &self.engines {
            if let Some(listing) = self.readout_of(engine) {
                let engine_containers = EngineContainers {
                    socket_path: &engine.socket_path,
                    owner: &engine.owner,
                    listing,
                };
                running_rows.extend(engine_containers.running_rows(
                    container_hover_target,
                    EngineContainerInteraction::SwitchContainer,
                    clock,
                    locale,
                ));
                stopped_rows.extend(engine_containers.stopped_rows(
                    container_hover_target,
                    EngineContainerInteraction::SwitchContainer,
                    clock,
                    locale,
                ));
            } else if let Some(read_failure) = self.readout_failure(engine) {
                failure_rows.push(GroupItem::reading(
                    ReadingRow::evaluated(
                        UnreadableEngineRow {
                            engine,
                            read_failure,
                        },
                        clock,
                        locale,
                    ),
                    DockerEngineTarget::from(engine),
                ));
            } else {
                running_rows.extend(self.listed_rows(engine, clock, locale));
            }
        }
        running_rows.extend(stopped_rows);
        failure_rows.extend(rows_ending_in_hidden_count(
            running_rows,
            DockerEngineTarget::HiddenContainers.into(),
            clock,
            locale,
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

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Presence",
            dictionary_table(
                self.probe_presence_count()
                    .map(|presence_count| {
                        vec![
                            LabeledValue::new("Present", presence_count.to_string()),
                            LabeledValue::new("Engines", self.engines.len().to_string()),
                        ]
                    })
                    .unwrap_or_default(),
            ),
        )]
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
    fn a_denied_docker_socket_shows_a_failure_row() {
        let clock = Clock::real();
        let locale = Locale::default();
        let rootful_socket = PathBuf::from(ROOTFUL_SOCKET_PATH);
        let mut group = rootful_engine_group();
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket.clone(),
                delivered(Err(EngineReadError::SocketDenied)),
            ),
            Clock::real(),
        );
        assert_eq!(group.container_rows(clock, locale).len(), 1);
        assert!(
            !group
                .hover_card_for(&DockerEngineTarget::from(&group.engines[0]), Clock::real())
                .is_empty()
        );
    }

    #[test]
    fn a_missing_docker_socket_shows_no_row() {
        let clock = Clock::real();
        let locale = Locale::default();
        let rootful_socket = PathBuf::from(ROOTFUL_SOCKET_PATH);
        let mut group = rootful_engine_group();
        let _ = group.update(
            DockerEnginesMessage::Engine(
                rootful_socket.clone(),
                SnapshotDelivery::SourceUnavailable,
            ),
            Clock::real(),
        );
        assert!(group.group_content(clock, locale).is_empty());
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
        assert!(!group.group_content(clock, locale).is_empty());
    }
}
