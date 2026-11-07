use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use chrono::Locale;
use iced::{Subscription, alignment::Horizontal};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    podman_container_engine::{
        libpod_running_containers::{
            EngineContainers, EngineReadError, EngineReadout, RunningContainer,
        },
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
        hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::{approximate_memory_text, cpu_share_quantity},
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        discovered_systemd_services::{DiscoveredSystemdServices, ScopedServiceCommands},
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodmanEngineTarget {
    socket_path: PathBuf,
}

impl From<&DiscoveredPodmanEngine> for PodmanEngineTarget {
    fn from(engine: &DiscoveredPodmanEngine) -> Self {
        Self {
            socket_path: engine.socket_path.clone(),
        }
    }
}

impl From<PodmanEngineTarget> for HoverTarget {
    fn from(target: PodmanEngineTarget) -> Self {
        HoverTarget::PodmanEngine(target)
    }
}

impl IntoTableRecord<3> for &RunningContainer {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(
            self.shown_name.clone(),
            [
                self.image.clone().into(),
                cpu_share_quantity(self.cpu_load)
                    .map_or_else(|| ValueCell::from(String::new()), ValueCell::from),
                approximate_memory_text(self.resident_memory).into(),
            ],
        )
    }
}

struct PodmanEngineRow<'a> {
    engine: &'a DiscoveredPodmanEngine,
    readout: &'a EngineReadout,
}

fn running_summary(engine: &EngineContainers) -> String {
    format!("{} running", engine.containers.len())
}

fn containers_section(engine: &EngineContainers) -> Section {
    Section::column_table(
        [
            ColumnHeading::titled("Image").align_x(Horizontal::Left),
            ColumnHeading::titled("CPU"),
            ColumnHeading::titled("Memory"),
        ],
        engine.containers.iter(),
    )
}

impl GroupItemReading for PodmanEngineRow<'_> {
    fn name(&self) -> &'static str {
        "Podman engine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.engine.owner.to_string().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Cube(Weight::Regular).into()
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
            Section::dictionary([
                LabeledValue::new("Unit", self.engine.unit.unit_name.clone()),
                LabeledValue::new("Socket", self.engine.socket_path.display().to_string()),
            ]),
        ])
    }
}

fn is_hidden_engine(readout: &EngineReadout) -> bool {
    matches!(
        readout,
        EngineReadout::Unreadable(EngineReadError::SocketMissing)
    )
}

#[derive(Default)]
pub struct PodmanEnginesGroup {
    services: DiscoveredSystemdServices,
    engines: Vec<DiscoveredPodmanEngine>,
    readouts: BTreeMap<PathBuf, LatestSnapshot<EngineReadout, HidesFetchFailures>>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PodmanEnginesMessage {
    Services(ScopedServiceCommands),
    Engine(PathBuf, SnapshotDelivery<EngineReadout>),
    Expansion(WidgetGroupExpansionMessage),
}

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

    pub fn update(&mut self, message: PodmanEnginesMessage, clock: Clock) {
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
            PodmanEnginesMessage::Expansion(message) => {
                self.expansion.update(message);
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &PodmanEngineTarget, clock: Clock) -> Option<HoverCard> {
        let engine = self
            .engines
            .iter()
            .find(|engine| engine.socket_path == target.socket_path)?;
        let reading = self.readouts.get(&target.socket_path)?;
        let row = PodmanEngineRow {
            engine,
            readout: reading.get()?,
        };
        reading.hover_card_with(row.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, PodmanEnginesMessage> {
        let rows = self.engines.iter().filter_map(|engine| {
            let readout = self.readouts.get(&engine.socket_path)?.get()?;
            (!is_hidden_engine(readout)).then(|| {
                GroupItem::reading(
                    PodmanEngineRow { engine, readout },
                    PodmanEngineTarget::from(engine),
                )
            })
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::PodmanEngines,
            &self.expansion,
            PodmanEnginesMessage::Expansion,
            rows,
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Cube(Weight::Regular).into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_socket_is_hidden_but_a_denied_one_is_not() {
        assert!(is_hidden_engine(&EngineReadout::Unreadable(
            EngineReadError::SocketMissing
        )));
        assert!(!is_hidden_engine(&EngineReadout::Unreadable(
            EngineReadError::SocketDenied
        )));
    }

    #[test]
    fn an_idle_engine_reports_zero_running_without_usage() {
        assert_eq!(
            running_summary(&EngineContainers {
                containers: Vec::new()
            }),
            "0 running"
        );
    }
}
