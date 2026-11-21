use std::{collections::BTreeMap, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    container_power::ContainerEngineApi, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    podman_container_engine::{
        libpod_running_containers::{EngineListing, EngineReadError},
        systemd_service_engine_discovery::DiscoveredPodmanEngine,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    indicator_groups::{
        engine_container_indicators::{
            EngineContainerInteraction, EngineContainers, RunningContainerIndicator,
            StoppedContainerIndicator, container_switch_task, engine_failure_label,
            indicators_ending_in_hidden_count,
        },
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::{
        discovered_systemd_services::{DiscoveredSystemdServices, ScopedServiceCommands},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

struct UnreadableEngineIndicator<'view> {
    engine: &'view DiscoveredPodmanEngine,
    read_failure: &'view EngineReadError,
}

impl<'view> UnreadableEngineIndicator<'view> {
    fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Error",
                SectionWidget::paragraph(self.read_failure.to_string()),
            ),
            Section::new(
                "Engine",
                dictionary_table([
                    LabeledValue::new("Unit", self.engine.unit.unit_name.clone()),
                    LabeledValue::new("Socket", self.engine.socket_path.display().to_string()),
                ]),
            ),
        ]
    }

    fn indicator(self) -> GroupIndicator<'view> {
        let label = ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned());
        let base = GroupIndicator::glyph_only(
            self.engine.owner.to_string(),
            Phosphor::Cube(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_hover_card(Box::new(move |_| self.hover_card()));
        base.with_label(label)
    }
}

#[derive(Default)]
pub struct PodmanEnginesGroup {
    services: DiscoveredSystemdServices,
    engines: Vec<DiscoveredPodmanEngine>,
    readouts: BTreeMap<PathBuf, SubscribedSnapshot<EngineListing>>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum PodmanEnginesMessage {
    Services(ScopedServiceCommands),
    Engine(PathBuf, SnapshotDelivery<EngineListing>),
    PowerChangeRefused(GuestPowerError),
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
                    snapshot_subscription::<EngineListing>(engine.socket_path.clone())
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
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: PodmanEnginesInteraction,
    ) -> Task<PodmanEnginesMessage> {
        match interaction {
            EngineContainerInteraction::SwitchContainer(request) => container_switch_task(
                ContainerEngineApi::Libpod,
                request,
                PodmanEnginesMessage::PowerChangeRefused,
            ),
            EngineContainerInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn running_container_indicators<'view>(
        engine: &'view DiscoveredPodmanEngine,
        listing: &'view EngineListing,
        sample: &'view SubscribedSnapshot<EngineListing>,
    ) -> Vec<GroupIndicator<'view>> {
        let engine_containers = EngineContainers {
            socket_path: &engine.socket_path,
            owner: &engine.owner,
            listing,
        };
        engine_containers
            .running_indicators(|request| {
                IndicatorGroupInteraction::PodmanEngines(
                    EngineContainerInteraction::SwitchContainer(request),
                )
            })
            .into_iter()
            .zip(&listing.running.containers)
            .map(|(row, container)| {
                row.with_hover_card(Box::new(move |clock| {
                    sample.hover_card_with(
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
        engine: &'view DiscoveredPodmanEngine,
        listing: &'view EngineListing,
        sample: &'view SubscribedSnapshot<EngineListing>,
    ) -> Vec<GroupIndicator<'view>> {
        let engine_containers = EngineContainers {
            socket_path: &engine.socket_path,
            owner: &engine.owner,
            listing,
        };
        engine_containers
            .stopped_indicators(|request| {
                IndicatorGroupInteraction::PodmanEngines(
                    EngineContainerInteraction::SwitchContainer(request),
                )
            })
            .into_iter()
            .zip(&listing.stopped)
            .map(|(row, container)| {
                row.with_hover_card(Box::new(move |_| {
                    sample.hover_card_with(
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

    fn container_indicators(&self) -> Vec<GroupIndicator<'_>> {
        let mut failure_indicators = Vec::new();
        let mut running_indicators = Vec::new();
        let mut stopped_indicators = Vec::new();
        for engine in &self.engines {
            let Some(sample) = self.readouts.get(&engine.socket_path) else {
                continue;
            };
            if let Some(listing) = sample.get() {
                running_indicators
                    .extend(Self::running_container_indicators(engine, listing, sample));
                stopped_indicators
                    .extend(Self::stopped_container_indicators(engine, listing, sample));
            } else if let Some(read_failure) = sample.fetch_failure() {
                failure_indicators.push(
                    UnreadableEngineIndicator {
                        engine,
                        read_failure,
                    }
                    .indicator(),
                );
            }
        }
        running_indicators.extend(stopped_indicators);
        failure_indicators.extend(indicators_ending_in_hidden_count(running_indicators));
        failure_indicators
    }

    #[must_use]
    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::PodmanEngines,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::PodmanEngines.title(),
                IconGlyph::SimpleIcon(SimpleIcon::Podman),
                IndicatorState::Idle,
            ),
            &self.expansion,
            |expansion| {
                IndicatorGroupInteraction::PodmanEngines(EngineContainerInteraction::Expansion(
                    expansion,
                ))
            },
            self.container_indicators(),
        )
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
                    Ok(EngineListing {
                        running: EngineContainers {
                            containers: Vec::new(),
                        },
                        stopped: (0..20).map(stopped_container).collect(),
                    }),
                    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(
                        5,
                    ))),
                ),
            ),
            Clock::real(),
        );
        let rows = group.container_indicators();
        assert_eq!(rows.len(), 12);
        assert_eq!(
            rows.last().map(GroupIndicator::press_actions),
            Some(Vec::new())
        );
    }
}
