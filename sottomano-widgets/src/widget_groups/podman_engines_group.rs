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
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    widget_groups::{
        engine_container_rows::{
            EngineContainerInteraction, EngineContainers, RunningContainerRow, StoppedContainerRow,
            container_switch_task, engine_failure_label, rows_ending_in_hidden_count,
        },
        group_summary::GroupSummary,
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion},
        widget_group_identity::WidgetGroupFamily,
        widget_group_interaction::WidgetGroupInteraction,
    },
    widgets::{
        discovered_systemd_services::{DiscoveredSystemdServices, ScopedServiceCommands},
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

struct UnreadableEngineRow<'a> {
    engine: &'a DiscoveredPodmanEngine,
    read_failure: &'a EngineReadError,
}

impl<'a> UnreadableEngineRow<'a> {
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

    fn reading_row(self) -> ReadingRow<'a> {
        let label = ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned());
        let base = ReadingRow::glyph_only(
            self.engine.owner.to_string(),
            Phosphor::Cube(Weight::Regular).into(),
            IndicatorState::Failed,
            Box::new(move |_| self.hover_card()),
        );
        ReadingRow {
            readout: IndicatorReadout {
                label,
                ..base.readout
            },
            ..base
        }
    }
}

#[derive(Default)]
pub struct PodmanEnginesGroup {
    services: DiscoveredSystemdServices,
    engines: Vec<DiscoveredPodmanEngine>,
    readouts: BTreeMap<PathBuf, SubscribedSnapshot<EngineListing>>,
    expansion: WidgetGroupExpansion,
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

    fn running_container_rows<'a>(
        engine: &'a DiscoveredPodmanEngine,
        listing: &'a EngineListing,
        sample: &'a SubscribedSnapshot<EngineListing>,
    ) -> Vec<GroupIndicator<'a>> {
        let engine_containers = EngineContainers {
            socket_path: &engine.socket_path,
            owner: &engine.owner,
            listing,
        };
        engine_containers
            .running_rows(|request| {
                WidgetGroupInteraction::PodmanEngines(EngineContainerInteraction::SwitchContainer(
                    request,
                ))
            })
            .into_iter()
            .zip(&listing.running.containers)
            .map(|(row, container)| {
                row.with_hover_card(Box::new(move |clock| {
                    sample.hover_card_with(
                        RunningContainerRow {
                            container,
                            owner: &engine.owner,
                        }
                        .hover_card(clock),
                    )
                }))
            })
            .collect()
    }

    fn stopped_container_rows<'a>(
        engine: &'a DiscoveredPodmanEngine,
        listing: &'a EngineListing,
        sample: &'a SubscribedSnapshot<EngineListing>,
    ) -> Vec<GroupIndicator<'a>> {
        let engine_containers = EngineContainers {
            socket_path: &engine.socket_path,
            owner: &engine.owner,
            listing,
        };
        engine_containers
            .stopped_rows(|request| {
                WidgetGroupInteraction::PodmanEngines(EngineContainerInteraction::SwitchContainer(
                    request,
                ))
            })
            .into_iter()
            .zip(&listing.stopped)
            .map(|(row, container)| {
                row.with_hover_card(Box::new(move |_| {
                    sample.hover_card_with(
                        StoppedContainerRow {
                            container,
                            owner: &engine.owner,
                        }
                        .hover_card(),
                    )
                }))
            })
            .collect()
    }

    fn container_rows(&self) -> Vec<GroupIndicator<'_>> {
        let mut failure_rows = Vec::new();
        let mut running_rows = Vec::new();
        let mut stopped_rows = Vec::new();
        for engine in &self.engines {
            let Some(sample) = self.readouts.get(&engine.socket_path) else {
                continue;
            };
            if let Some(listing) = sample.get() {
                running_rows.extend(Self::running_container_rows(engine, listing, sample));
                stopped_rows.extend(Self::stopped_container_rows(engine, listing, sample));
            } else if let Some(read_failure) = sample.fetch_failure() {
                failure_rows.push(GroupIndicator::sample(
                    UnreadableEngineRow {
                        engine,
                        read_failure,
                    }
                    .reading_row(),
                ));
            }
        }
        running_rows.extend(stopped_rows);
        failure_rows.extend(rows_ending_in_hidden_count(running_rows));
        failure_rows
    }

    #[must_use]
    pub fn group_content(&self) -> WidgetGroupContent<'_> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PodmanEngines,
            GroupSummary::glyph(
                IconGlyph::SimpleIcon(SimpleIcon::Podman),
                IndicatorState::Idle,
            ),
            &self.expansion,
            |expansion| {
                WidgetGroupInteraction::PodmanEngines(EngineContainerInteraction::Expansion(
                    expansion,
                ))
            },
            self.container_rows(),
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
        let rows = group.container_rows();
        assert_eq!(rows.len(), 12);
        assert_eq!(
            rows.last().map(GroupIndicator::press_actions),
            Some(Vec::new())
        );
    }
}
