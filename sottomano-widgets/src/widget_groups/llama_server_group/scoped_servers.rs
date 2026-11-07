use std::{collections::BTreeMap, mem::take, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::llama_server_slots::{
        LlamaServer, LlamaServerOnBus, LlamaServerUnit, ModelInstance,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        SystemdOnBus, SystemdUnitScope, service_commands::ServiceCommands,
        unit_active_state::UnitActivity,
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
        group_item_reading::GroupItemReading,
        unit_switch::{ServiceReadiness, UnitSwitch, UnitSwitchMessage},
    },
    hover_cards::hover_card_body::{
        HoverCardBody, Section,
        hover_card_source_delivery::{HoverCard, SourcedSections},
    },
    widgets::{
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        value_cells::{LabeledValue, ValueCell},
    },
};

struct ServerReadings {
    discovered: LlamaServerUnit,
    switch: UnitSwitch,
    server: LatestSnapshot<LlamaServer, HidesFetchFailures>,
}

impl ServerReadings {
    fn new(discovered: LlamaServerUnit) -> Self {
        Self {
            switch: UnitSwitch::new(discovered.unit.clone(), SERVER_SWITCH),
            discovered,
            server: LatestSnapshot::default(),
        }
    }
}

pub struct ScopedLlamaServers {
    scope: SystemdUnitScope,
    services: LatestSnapshot<ServiceCommands, HidesFetchFailures>,
    servers: BTreeMap<String, ServerReadings>,
}

#[derive(Debug, Clone)]
pub enum ScopedServersMessage {
    Services(SnapshotDelivery<ServiceCommands>),
    Switch(String, UnitSwitchMessage),
    Server(String, SnapshotDelivery<LlamaServer>),
}

impl ScopedLlamaServers {
    pub fn new(scope: SystemdUnitScope) -> Self {
        Self {
            scope,
            services: LatestSnapshot::default(),
            servers: BTreeMap::new(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<ScopedServersMessage> {
        let systemd = SystemdOnBus {
            buses: buses.clone(),
            scope: self.scope,
        };
        Subscription::batch(
            [
                snapshot_subscription::<ServiceCommands>(systemd)
                    .map(ScopedServersMessage::Services),
            ]
            .into_iter()
            .chain(self.servers.values().flat_map(|readings| {
                let unit = readings.discovered.unit.unit_name.clone();
                let server = LlamaServerOnBus {
                    buses: buses.clone(),
                    server: readings.discovered.clone(),
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
                    .unwrap_or_else(|| ServerReadings::new(discovered));
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
        model_groups: Vec<SourcedSections>,
        clock: Clock,
    ) -> Option<HoverCard> {
        let readings = self.servers.get(&target.unit)?;
        let reading = &readings.server;
        let server = reading.get();
        match target.reading {
            LlamaServerReading::Switch => readings.switch.hover_card(
                server.map_or(ServiceReadiness::NotYetServing, |_| {
                    ServiceReadiness::Serving
                }),
                clock,
            ),
            LlamaServerReading::Model => {
                let server = server?;
                let resident = server.resident_model();
                let endpoint = &readings.discovered.endpoint;
                let status_and_web_ui = Section::dictionary(
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
                );
                let available_models = Section::dictionary(
                    server
                        .models
                        .iter()
                        .filter(|model| Some(*model) != resident)
                        .map(|model| LabeledValue::new(model.id.clone(), model.status)),
                )
                .titled("Available models");
                Some(
                    reading
                        .hover_card_with(HoverCardBody::sections([
                            status_and_web_ui,
                            available_models,
                        ]))?
                        .with_groups(model_groups),
                )
            }
            LlamaServerReading::GenerationRate => {
                reading.hover_card_with(GenerationRate(server?.generation_rate?).hover_card(clock))
            }
            LlamaServerReading::Slots => {
                reading.hover_card_with(SlotOccupancy(&server?.slots).hover_card(clock))
            }
            LlamaServerReading::ContextFill => reading
                .hover_card_with(ContextFill::of_busy_slots(&server?.slots)?.hover_card(clock)),
        }
    }

    pub fn items<'a, M: 'a>(
        &'a self,
        lift: fn(ScopedServersMessage) -> M,
    ) -> impl Iterator<Item = GroupItem<'a, M>> + 'a {
        self.servers.values().flat_map(move |readings| {
            let unit = &readings.discovered.unit.unit_name;
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
                        ServedModel { resident },
                        target(LlamaServerReading::Model),
                    );
                    match resident {
                        Some(_) => model_item.previewed_on_handle(),
                        None => model_item,
                    }
                }),
                busy.and_then(|server| server.generation_rate).map(|rate| {
                    GroupItem::reading(
                        GenerationRate(rate),
                        target(LlamaServerReading::GenerationRate),
                    )
                }),
                running
                    .filter(|server| !server.slots.is_empty())
                    .map(|server| {
                        GroupItem::reading(
                            SlotOccupancy(&server.slots),
                            target(LlamaServerReading::Slots),
                        )
                    }),
                busy.and_then(|server| ContextFill::of_busy_slots(&server.slots))
                    .map(|fill| GroupItem::reading(fill, target(LlamaServerReading::ContextFill))),
                readings.switch.group_item(
                    readings
                        .server
                        .get()
                        .map_or(ServiceReadiness::NotYetServing, |_| {
                            ServiceReadiness::Serving
                        }),
                    target(LlamaServerReading::Switch),
                    lift(ScopedServersMessage::Switch(
                        unit.clone(),
                        UnitSwitchMessage::Press,
                    )),
                ),
            ]
            .into_iter()
            .flatten()
        })
    }
}
