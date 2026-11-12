use std::{
    borrow::Cow,
    collections::BTreeMap,
    convert::identity,
    path::{Path, PathBuf},
};

use chrono::Locale;
use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    docker_container_engine::{
        DockerEngineListing, DockerEngineProbe, DockerEngineReadout, DockerEngineSocket,
        candidate_docker_sockets,
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
        podman_engines_group::{containers_section, running_summary},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
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
pub struct DockerEngineTarget {
    socket_path: Cow<'static, str>,
}

impl DockerEngineTarget {
    #[must_use]
    pub const fn of_socket(socket_path: Cow<'static, str>) -> Self {
        Self { socket_path }
    }

    fn names_socket(&self, socket_path: &Path) -> bool {
        Path::new(self.socket_path.as_ref()) == socket_path
    }
}

impl From<DockerEngineTarget> for HoverTarget {
    fn from(target: DockerEngineTarget) -> Self {
        HoverTarget::DockerEngine(target)
    }
}

struct DockerEngineRow<'a> {
    engine: &'a DockerEngineSocket,
    readout: &'a EngineReadout,
}

impl GroupItemReading for DockerEngineRow<'_> {
    fn name(&self) -> &'static str {
        "Docker engine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.engine.owner.to_string().into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Docker)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(match self.readout {
            EngineReadout::Containers(engine) => running_summary(engine),
            EngineReadout::Unreadable(EngineReadError::SocketDenied) => {
                "no access: socket denied".to_owned()
            }
            EngineReadout::Unreadable(_) => "unreadable".to_owned(),
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.readout {
            EngineReadout::Containers(_) => ReadingDisplayState::Idle,
            EngineReadout::Unreadable(_) => ReadingDisplayState::Failed,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let readout_section = match self.readout {
            EngineReadout::Containers(engine) if engine.containers.is_empty() => {
                Section::note("No running containers")
            }
            EngineReadout::Containers(engine) => containers_section(engine),
            EngineReadout::Unreadable(failure) => Section::error(failure),
        };
        HoverCardBody::sections([
            readout_section,
            Section::dictionary([LabeledValue::new(
                "Socket",
                self.engine.socket_path.display().to_string(),
            )]),
        ])
    }
}

struct DockerEngineListingRow<'a> {
    engine: &'a DockerEngineSocket,
    listing: &'a DockerEngineListing,
}

impl GroupItemReading for DockerEngineListingRow<'_> {
    fn name(&self) -> &'static str {
        "Docker engine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.engine.owner.to_string().into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Docker)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} running", self.listing.running_count()))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.listing.running_count() > 0 {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let mut detail_entries = vec![LabeledValue::new(
            "Running",
            self.listing.running_count().to_string(),
        )];
        detail_entries.extend(
            self.listing
                .listings
                .iter()
                .map(|listed| LabeledValue::new("Container", listed.container_name.clone())),
        );
        detail_entries.push(LabeledValue::new(
            "Socket",
            self.engine.socket_path.display().to_string(),
        ));
        HoverCardBody::dictionary(detail_entries)
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
    Expansion(WidgetGroupExpansionMessage),
}

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

    pub fn update(&mut self, message: DockerEnginesMessage, clock: Clock) {
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
            DockerEnginesMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
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

    fn readout_of(&self, engine: &DockerEngineSocket) -> Option<&EngineReadout> {
        self.readouts
            .get(&engine.socket_path)?
            .get()
            .map(|DockerEngineReadout(readout)| readout)
    }

    fn listing_of(&self, engine: &DockerEngineSocket) -> Option<&DockerEngineListing> {
        self.listings.get(&engine.socket_path)?.get()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &DockerEngineTarget, clock: Clock) -> Option<HoverCard> {
        let engine = self
            .engines
            .iter()
            .find(|engine| target.names_socket(&engine.socket_path))?;
        if let Some(readout) = self.readout_of(engine) {
            return self
                .readouts
                .get(&engine.socket_path)?
                .hover_card_with(DockerEngineRow { engine, readout }.hover_card(clock));
        }
        let listing = self.listing_of(engine)?;
        self.listings
            .get(&engine.socket_path)?
            .hover_card_with(DockerEngineListingRow { engine, listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let rows = self.engine_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::DockerEngines,
            &self.expansion,
            identity,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: IconGlyph::SimpleIcon(SimpleIcon::Docker),
            reading: self,
        })
    }

    fn engine_rows(&self) -> Vec<GroupItem<'_, WidgetGroupExpansionMessage>> {
        let mut detailed_engines = 0;
        let mut rows = Vec::new();
        for engine in &self.engines {
            if let Some(readout) = self.readout_of(engine) {
                let socket_missing = matches!(
                    readout,
                    EngineReadout::Unreadable(EngineReadError::SocketMissing)
                );
                if !socket_missing {
                    rows.push(GroupItem::reading(
                        DockerEngineRow { engine, readout },
                        DockerEngineTarget::of_socket(Cow::Owned(
                            engine.socket_path.display().to_string(),
                        )),
                    ));
                }
                detailed_engines += 1;
            }
        }
        if detailed_engines > 0 {
            return rows;
        }
        self.engines
            .iter()
            .filter_map(|engine| {
                let listing = self.listing_of(engine)?;
                Some(GroupItem::reading(
                    DockerEngineListingRow { engine, listing },
                    DockerEngineTarget::of_socket(Cow::Owned(
                        engine.socket_path.display().to_string(),
                    )),
                ))
            })
            .collect()
    }
}

impl GroupItemReading for DockerEnginesGroup {
    fn name(&self) -> &'static str {
        "Docker engines"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
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
    use std::time::Duration;

    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        podman_container_engine::{
            libpod_running_containers::EngineContainers,
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
        group.update(
            DockerEnginesMessage::Engine(
                rootful_socket.clone(),
                delivered(EngineReadout::Unreadable(EngineReadError::SocketMissing)),
            ),
            Clock::real(),
        );
        assert!(group.group_content().is_empty());
        group.update(
            DockerEnginesMessage::Engine(
                rootful_socket,
                delivered(EngineReadout::Containers(EngineContainers {
                    containers: Vec::new(),
                })),
            ),
            Clock::real(),
        );
        assert!(!group.group_content().is_empty());
    }
}
