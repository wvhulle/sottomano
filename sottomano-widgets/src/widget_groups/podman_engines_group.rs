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
    podman_container_engine::{
        libpod_running_containers::{EngineReadError, EngineReadout},
        systemd_service_engine_discovery::DiscoveredPodmanEngine,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, SectionWidget, dictionary_table},
    },
    widget_groups::{
        engine_container_rows::{
            EngineContainerInteraction, EngineContainerTarget, EngineContainers,
            container_switch_task, rows_ending_in_hidden_count,
        },
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion, WidgetGroupFace},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        discovered_systemd_services::{DiscoveredSystemdServices, ScopedServiceCommands},
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PodmanEngineTarget {
    Engine { socket_path: PathBuf },
    Container(EngineContainerTarget),
    HiddenContainers,
}

impl From<&DiscoveredPodmanEngine> for PodmanEngineTarget {
    fn from(engine: &DiscoveredPodmanEngine) -> Self {
        Self::Engine {
            socket_path: engine.socket_path.clone(),
        }
    }
}

impl From<PodmanEngineTarget> for HoverTarget {
    fn from(target: PodmanEngineTarget) -> Self {
        HoverTarget::PodmanEngine(target)
    }
}

fn container_hover_target(target: EngineContainerTarget) -> HoverTarget {
    PodmanEngineTarget::Container(target).into()
}

struct UnreadableEngineRow<'a> {
    engine: &'a DiscoveredPodmanEngine,
    failure: &'a EngineReadError,
}

impl GroupItemReading for UnreadableEngineRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        self.engine.owner.to_string().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Cube(Weight::Regular).into()
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

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![
            Section::new("Error", SectionWidget::paragraph(self.failure.to_string())),
            Section::new(
                "Engine",
                dictionary_table([
                    LabeledValue::new("Unit", self.engine.unit.unit_name.clone()),
                    LabeledValue::new("Socket", self.engine.socket_path.display().to_string()),
                ]),
            ),
        ]
    }
}

struct PodmanEngineReading<'a> {
    engine: &'a DiscoveredPodmanEngine,
    reading: &'a SubscribedSnapshot<EngineReadout>,
}

#[derive(Default)]
pub struct PodmanEnginesGroup {
    services: DiscoveredSystemdServices,
    engines: Vec<DiscoveredPodmanEngine>,
    readouts: BTreeMap<PathBuf, SubscribedSnapshot<EngineReadout>>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum PodmanEnginesMessage {
    Services(ScopedServiceCommands),
    Engine(PathBuf, SnapshotDelivery<EngineReadout>),
    PowerChangeRefused(String),
    Interaction(PodmanEnginesInteraction),
}

pub type PodmanEnginesInteraction = EngineContainerInteraction;

impl PodmanEnginesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<PodmanEnginesMessage> {
        Subscription::batch(
            [DiscoveredSystemdServices::subscription(buses).map(PodmanEnginesMessage::Services)]
                .into_iter()
                .chain(self.engines.iter().map(|engine| {
                    snapshot_subscription::<EngineReadout>(engine.socket_path.clone())
                        .with(engine.socket_path.clone())
                        .map(|(socket_path, delivery)| {
                            PodmanEnginesMessage::Engine(socket_path, delivery)
                        })
                })),
        )
    }

    fn adopt_discovered(&mut self) {
        let mut engines: Vec<DiscoveredPodmanEngine> = self
            .services
            .discovered(DiscoveredPodmanEngine::discovered)
            .collect();
        engines.dedup_by(|later, earlier| later.socket_path == earlier.socket_path);
        self.readouts.retain(|socket_path, _| {
            engines
                .iter()
                .any(|engine| &engine.socket_path == socket_path)
        });
        self.engines = engines;
    }

    pub fn update(
        &mut self,
        message: PodmanEnginesMessage,
        clock: Clock,
    ) -> Task<PodmanEnginesMessage> {
        match message {
            PodmanEnginesMessage::Services(scoped_commands) => {
                self.services.receive(scoped_commands, clock);
                self.adopt_discovered();
            }
            PodmanEnginesMessage::Engine(socket_path, delivery) => {
                self.readouts
                    .entry(socket_path)
                    .or_default()
                    .receive(delivery, clock);
            }
            PodmanEnginesMessage::PowerChangeRefused(refusal) => {
                tracing::warn!(%refusal, "podman container power change refused");
            }
            PodmanEnginesMessage::Interaction(EngineContainerInteraction::SwitchContainer(
                request,
            )) => {
                return container_switch_task(
                    ContainerEngineApi::Libpod,
                    request,
                    PodmanEnginesMessage::PowerChangeRefused,
                );
            }
            PodmanEnginesMessage::Interaction(EngineContainerInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    fn engine_reading(&self, socket_path: &Path) -> Option<PodmanEngineReading<'_>> {
        let engine = self
            .engines
            .iter()
            .find(|engine| engine.socket_path == socket_path)?;
        Some(PodmanEngineReading {
            engine,
            reading: self.readouts.get(socket_path)?,
        })
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &PodmanEngineTarget, clock: Clock) -> Vec<Section> {
        match target {
            PodmanEngineTarget::Engine { socket_path } => {
                let Some(PodmanEngineReading { engine, reading }) =
                    self.engine_reading(socket_path)
                else {
                    return Vec::new();
                };
                let Some(EngineReadout::Unreadable(failure)) = reading.get() else {
                    return Vec::new();
                };
                reading.hover_card_with(UnreadableEngineRow { engine, failure }.hover_card(clock))
            }
            PodmanEngineTarget::Container(container_target) => {
                let Some(PodmanEngineReading { engine, reading }) =
                    self.engine_reading(Path::new(container_target.socket_path.as_ref()))
                else {
                    return Vec::new();
                };
                let Some(EngineReadout::Containers(listing)) = reading.get() else {
                    return Vec::new();
                };
                let engine_containers = EngineContainers {
                    socket_path: &engine.socket_path,
                    owner: &engine.owner,
                    listing,
                };
                let Some(container_body) =
                    engine_containers.hover_body_of(&container_target.container_id, clock)
                else {
                    return Vec::new();
                };
                reading.hover_card_with(container_body)
            }
            PodmanEngineTarget::HiddenContainers => Vec::new(),
        }
    }

    fn container_rows(&self) -> Vec<GroupItem<'_, PodmanEnginesInteraction>> {
        let mut failure_rows = Vec::new();
        let mut running_rows = Vec::new();
        let mut stopped_rows = Vec::new();
        for engine in &self.engines {
            let Some(readout) = self
                .readouts
                .get(&engine.socket_path)
                .and_then(SubscribedSnapshot::get)
            else {
                continue;
            };
            match readout {
                EngineReadout::Containers(listing) => {
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
                EngineReadout::Unreadable(EngineReadError::SocketMissing) => {}
                EngineReadout::Unreadable(failure) => failure_rows.push(GroupItem::reading(
                    UnreadableEngineRow { engine, failure },
                    PodmanEngineTarget::from(engine),
                )),
            }
        }
        running_rows.extend(stopped_rows);
        failure_rows.extend(rows_ending_in_hidden_count(
            running_rows,
            PodmanEngineTarget::HiddenContainers.into(),
        ));
        failure_rows
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, PodmanEnginesInteraction> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PodmanEngines,
            &self.expansion,
            EngineContainerInteraction::Expansion,
            self.container_rows(),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Cube(Weight::Regular).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{Duration, UNIX_EPOCH},
    };

    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        podman_container_engine::{
            libpod_running_containers::{EngineContainers, EngineListing, StoppedContainer},
            systemd_service_engine_discovery::EngineOwner,
        },
        systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    };

    use super::*;

    fn stopped_container(ordinal: u64) -> StoppedContainer {
        StoppedContainer {
            id: format!("container-{ordinal}"),
            container_name: format!("container-{ordinal}"),
            shown_name: format!("container-{ordinal}"),
            image: "alpine".to_owned(),
            stopped_at: UNIX_EPOCH + Duration::from_secs(ordinal),
        }
    }

    #[test]
    fn a_long_container_list_ends_in_a_hidden_count() {
        let socket_path = PathBuf::from("/run/podman/podman.sock");
        let mut group = PodmanEnginesGroup {
            engines: vec![DiscoveredPodmanEngine {
                unit: ScopedSystemdUnit {
                    scope: SystemdUnitScope::SystemWide,
                    unit_name: "podman.service".to_owned(),
                },
                owner: EngineOwner::Root,
                socket_path: socket_path.clone(),
            }],
            ..PodmanEnginesGroup::default()
        };
        let _ = group.update(
            PodmanEnginesMessage::Engine(
                socket_path,
                SnapshotDelivery::fixture_delivered(
                    Ok(EngineReadout::Containers(EngineListing {
                        running: EngineContainers {
                            containers: Vec::new(),
                        },
                        stopped: (0..20).map(stopped_container).collect(),
                    })),
                    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(
                        5,
                    ))),
                ),
            ),
            Clock::real(),
        );
        let rows = group.container_rows();
        assert_eq!(rows.len(), 12);
        assert_eq!(
            rows.last()
                .map(GroupItem::announcement)
                .and_then(|announcement| announcement.press_hints.left),
            None
        );
    }
}
