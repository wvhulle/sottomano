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
use uom::si::f64::Frequency;

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
    fn model_hover_card(&self, model_sections: Vec<Section>) -> Vec<Section> {
        let Some(server) = self.server.get() else {
            return Vec::new();
        };
        let resident = server.resident_model();
        let endpoint = &self.discovered.endpoint;
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
        let mut sections = self
            .server
            .hover_card_with(vec![status_and_web_ui, available_models]);
        sections.extend(model_sections);
        sections
    }

    fn service_readiness(&self) -> ServiceReadiness {
        self.server
            .get()
            .map_or(ServiceReadiness::NotYetServing, |_| {
                ServiceReadiness::Serving
            })
    }

    fn generation_rate_hover_card(&self, rate: Frequency) -> Vec<Section> {
        self.server.hover_card_with(
            GenerationRate {
                rate,
                server: &self.server_label,
            }
            .hover_card(),
        )
    }

    fn slot_occupancy_hover_card(&self, server: &LlamaServer) -> Vec<Section> {
        self.server.hover_card_with(
            SlotOccupancy {
                slots: &server.slots,
                server: &self.server_label,
            }
            .hover_card(),
        )
    }

    fn context_fill_hover_card(&self, fill: &ContextFill) -> Vec<Section> {
        self.server.hover_card_with(fill.hover_card())
    }

    fn switch_hover_card(&self) -> Vec<Section> {
        self.switch.hover_card(self.service_readiness())
    }

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

    pub fn items<'a, M: 'a>(
        &'a self,
        lift: fn(ScopedServersInteraction) -> M,
        model_sections: impl Fn() -> Vec<Section> + Copy + 'a,
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
                    )
                    .with_hover_card(Box::new(move |_| {
                        readings.model_hover_card(model_sections())
                    }));
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
                    .with_hover_card(Box::new(move |_| readings.generation_rate_hover_card(rate)))
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
                        .with_hover_card(Box::new(move |_| {
                            readings.slot_occupancy_hover_card(server)
                        }))
                    }),
                busy.and_then(|server| ContextFill::fullest_busy_slot(&server.slots, server_label))
                    .map(|fill| {
                        GroupItem::reading(
                            fill.reading_row(),
                            target(LlamaServerReading::ContextFill),
                        )
                        .with_hover_card(Box::new(move |_| readings.context_fill_hover_card(&fill)))
                    }),
                readings
                    .switch
                    .group_item(
                        readings.service_readiness(),
                        target(LlamaServerReading::Switch),
                        lift(ScopedServersInteraction::SwitchPressed(unit.clone())),
                    )
                    .map(|switch_item| {
                        switch_item.with_hover_card(Box::new(move |_| readings.switch_hover_card()))
                    }),
            ]
            .into_iter()
            .flatten()
        })
    }
}
