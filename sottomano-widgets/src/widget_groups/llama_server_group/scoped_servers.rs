use std::{collections::BTreeMap, mem::take, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::llama_server_slots::{LlamaServer, LlamaServerUnit, ModelInstance},
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
    systemd_units_journal::{
        SystemdUnitScope, service_commands::ServiceCommands, unit_active_state::UnitActivity,
    },
};

use super::{
    LlamaServerReading, LlamaServerTarget, SERVER_SWITCH, context_fill_gauge::ContextFill,
    generation_rate_label::GenerationRate, loaded_model_label::ServedModel,
    slot_occupancy_gauge::SlotOccupancy,
};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        unit_switch::{ServiceReadiness, UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::{LabeledValue, ValueCell},
    },
};

struct ServerReadings {
    server_label: String,
    discovered: LlamaServerUnit,
    switch: UnitSwitch,
    server: SubscribedSnapshot<LlamaServer>,
}

impl ServerReadings {
    fn new(scope: SystemdUnitScope, discovered: LlamaServerUnit) -> Self {
        let unit_stem = discovered
            .unit
            .unit_name
            .trim_end_matches(".service")
            .to_owned();
        let server_label = match scope {
            SystemdUnitScope::UserSession => unit_stem,
            SystemdUnitScope::SystemWide => format!("system {unit_stem}"),
        };
        Self {
            switch: UnitSwitch::new(
                discovered.unit.clone(),
                UnitSwitchFace {
                    name: server_label.clone().into(),
                    ..SERVER_SWITCH
                },
            ),
            server_label,
            discovered,
            server: SubscribedSnapshot::default(),
        }
    }
}

pub struct ScopedLlamaServers {
    scope: SystemdUnitScope,
    services: SubscribedSnapshot<ServiceCommands>,
    servers: BTreeMap<String, ServerReadings>,
}

#[derive(Debug)]
pub enum ScopedServersMessage {
    Services(SnapshotDelivery<ServiceCommands>),
    Switch(String, UnitSwitchMessage),
    Server(String, SnapshotDelivery<LlamaServer>),
}

#[derive(Debug, Clone)]
pub enum ScopedServersInteraction {
    SwitchPressed(String),
}

impl ScopedLlamaServers {
    pub fn new(scope: SystemdUnitScope) -> Self {
        Self {
            scope,
            services: SubscribedSnapshot::default(),
            servers: BTreeMap::new(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<ScopedServersMessage> {
        let systemd = OnBus {
            buses: buses.clone(),
            inputs: self.scope,
        };
        Subscription::batch(
            [
                snapshot_subscription::<ServiceCommands>(systemd)
                    .map(ScopedServersMessage::Services),
            ]
            .into_iter()
            .chain(self.servers.values().flat_map(|readings| {
                let unit = readings.discovered.unit.unit_name.clone();
                let server = OnBus {
                    buses: buses.clone(),
                    inputs: readings.discovered.clone(),
                };
                [
                    readings
                        .switch
                        .subscription(buses)
                        .with(unit.clone())
                        .map(|(unit, message)| ScopedServersMessage::Switch(unit, message)),
                    snapshot_subscription::<LlamaServer>(server)
                        .with(unit)
                        .map(|(unit, delivery)| ScopedServersMessage::Server(unit, delivery)),
                ]
            })),
        )
    }

    fn adopt_discovered(&mut self) {
        let mut previous = take(&mut self.servers);
        self.servers = self
            .services
            .get()
            .into_iter()
            .flat_map(|services| services.services.iter())
            .filter_map(|command| LlamaServerUnit::discovered(self.scope, command))
            .map(|discovered| {
                let readings = previous
                    .remove(&discovered.unit.unit_name)
                    .filter(|readings| readings.discovered == discovered)
                    .unwrap_or_else(|| ServerReadings::new(self.scope, discovered));
                (readings.discovered.unit.unit_name.clone(), readings)
            })
            .collect();
    }

    pub fn update(
        &mut self,
        message: ScopedServersMessage,
        clock: Clock,
    ) -> Task<ScopedServersMessage> {
        match message {
            ScopedServersMessage::Services(delivery) => {
                self.services.receive(delivery, clock);
                self.adopt_discovered();
                Task::none()
            }
            ScopedServersMessage::Switch(unit, message) => match self.servers.get_mut(&unit) {
                Some(readings) => readings
                    .switch
                    .update(message, clock)
                    .map(move |message| ScopedServersMessage::Switch(unit.clone(), message)),
                None => Task::none(),
            },
            ScopedServersMessage::Server(unit, delivery) => {
                if let Some(readings) = self.servers.get_mut(&unit) {
                    readings.server.receive(delivery, clock);
                }
                Task::none()
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: ScopedServersInteraction,
    ) -> Task<ScopedServersMessage> {
        match interaction {
            ScopedServersInteraction::SwitchPressed(unit) => match self.servers.get_mut(&unit) {
                Some(readings) => readings
                    .switch
                    .press()
                    .map(move |message| ScopedServersMessage::Switch(unit.clone(), message)),
                None => Task::none(),
            },
        }
    }

    pub fn resident_instance(&self, unit: &str) -> Option<ModelInstance> {
        self.servers
            .get(unit)?
            .server
            .get()?
            .resident_model()?
            .instance
            .clone()
    }

    pub fn resident_model_file(&self, unit: &str) -> Option<PathBuf> {
        self.servers
            .get(unit)?
            .server
            .get()?
            .resident_model()?
            .model_file
            .clone()
    }

    pub fn hover_card_for(
        &self,
        target: &LlamaServerTarget,
        model_groups: Vec<Section>,
        clock: Clock,
    ) -> Vec<Section> {
        let Some(readings) = self.servers.get(&target.unit) else {
            return Vec::new();
        };
        let reading = &readings.server;
        let server = reading.get();
        match target.reading {
            LlamaServerReading::Switch => {
                readings
                    .switch
                    .hover_card(server.map_or(ServiceReadiness::NotYetServing, |_| {
                        ServiceReadiness::Serving
                    }))
            }
            LlamaServerReading::Model => {
                let Some(server) = server else {
                    return Vec::new();
                };
                let resident = server.resident_model();
                let endpoint = &readings.discovered.endpoint;
                let status_and_web_ui = Section::new(
                    "Status",
                    dictionary_table(
                        resident
                            .map(|model| LabeledValue::new("Status", model.status))
                            .into_iter()
                            .chain([LabeledValue::new(
                                "Web UI",
                                ValueCell::Link {
                                    label: endpoint.clone(),
                                    url: endpoint.clone(),
                                },
                            )]),
                    ),
                );
                let available_models = Section::new(
                    "Available models",
                    dictionary_table(
                        server
                            .models
                            .iter()
                            .filter(|model| Some(*model) != resident)
                            .map(|model| LabeledValue::new(model.id.clone(), model.status)),
                    ),
                );
                let mut sections =
                    reading.hover_card_with(vec![status_and_web_ui, available_models]);
                sections.extend(model_groups);
                sections
            }
            LlamaServerReading::GenerationRate => {
                let Some(rate) = server.and_then(|server| server.generation_rate) else {
                    return Vec::new();
                };
                reading.hover_card_with(
                    GenerationRate {
                        rate,
                        server: &readings.server_label,
                    }
                    .hover_card(clock),
                )
            }
            LlamaServerReading::Slots => {
                let Some(server) = server else {
                    return Vec::new();
                };
                reading.hover_card_with(
                    SlotOccupancy {
                        slots: &server.slots,
                        server: &readings.server_label,
                    }
                    .hover_card(clock),
                )
            }
            LlamaServerReading::ContextFill => {
                let Some(context_fill) = server.and_then(|server| {
                    ContextFill::fullest_busy_slot(&server.slots, &readings.server_label)
                }) else {
                    return Vec::new();
                };
                reading.hover_card_with(context_fill.hover_card(clock))
            }
        }
    }

    pub fn items<'a, M: 'a>(
        &'a self,
        lift: fn(ScopedServersInteraction) -> M,
    ) -> impl Iterator<Item = GroupItem<'a, M>> + 'a {
        self.servers.values().flat_map(move |readings| {
            let unit = &readings.discovered.unit.unit_name;
            let server_label = readings.server_label.as_str();
            let target = |reading| LlamaServerTarget {
                bus: self.scope.bus(),
                unit: unit.clone(),
                reading,
            };
            let running = Some(readings)
                .filter(|readings| readings.switch.activity() == Some(UnitActivity::Running))
                .and_then(|readings| readings.server.get());
            let busy = running.filter(|server| server.busy_slots().next().is_some());
            [
                running.map(|server| {
                    let resident = server.resident_model();
                    let model_item = GroupItem::reading(
                        ServedModel {
                            resident,
                            server: server_label,
                        }
                        .reading_row(),
                        target(LlamaServerReading::Model),
                    );
                    match resident {
                        Some(_) => model_item.previewed_on_handle(),
                        None => model_item,
                    }
                }),
                busy.and_then(|server| server.generation_rate).map(|rate| {
                    GroupItem::reading(
                        GenerationRate {
                            rate,
                            server: server_label,
                        }
                        .reading_row(),
                        target(LlamaServerReading::GenerationRate),
                    )
                }),
                running
                    .filter(|server| !server.slots.is_empty())
                    .map(|server| {
                        GroupItem::reading(
                            SlotOccupancy {
                                slots: &server.slots,
                                server: server_label,
                            }
                            .reading_row(),
                            target(LlamaServerReading::Slots),
                        )
                    }),
                busy.and_then(|server| ContextFill::fullest_busy_slot(&server.slots, server_label))
                    .map(|fill| {
                        GroupItem::reading(
                            fill.reading_row(),
                            target(LlamaServerReading::ContextFill),
                        )
                    }),
                readings.switch.group_item(
                    readings
                        .server
                        .get()
                        .map_or(ServiceReadiness::NotYetServing, |_| {
                            ServiceReadiness::Serving
                        }),
                    target(LlamaServerReading::Switch),
                    lift(ScopedServersInteraction::SwitchPressed(unit.clone())),
                ),
            ]
            .into_iter()
            .flatten()
        })
    }
}
