use std::{collections::BTreeMap, mem::take, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_sources::{
    SourceDelivery,
    ai_usage_spend::llama_server_slots::{
        LlamaServer, LlamaServerOnBus, LlamaServerUnit, ModelInstance,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{SystemdOnBus, SystemdUnitScope, service_commands::ServiceCommands},
};

use super::{
    LlamaServerReading, LlamaServerTarget, SERVER_SWITCH, context_fill_gauge::ContextFill,
    generation_rate_label::GenerationRate, loaded_model_label::ServedModel,
    slot_occupancy_gauge::SlotOccupancy,
};
use crate::{
    background_work::source_subscription,
    clock::Clock,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        tooltip_body::{Cell, KeyValue, Section, SourcedSections, Tooltip, TooltipBody},
        unit_switch::{UnitSwitch, UnitSwitchMessage},
    },
};

struct ServerReadings {
    discovered: LlamaServerUnit,
    switch: UnitSwitch,
    server: ContinuousReading<LlamaServer>,
}

impl ServerReadings {
    fn new(discovered: LlamaServerUnit) -> Self {
        Self {
            switch: UnitSwitch::new(discovered.unit.clone(), SERVER_SWITCH),
            discovered,
            server: ContinuousReading::default(),
        }
    }
}

pub struct ScopedLlamaServers {
    scope: SystemdUnitScope,
    services: ContinuousReading<ServiceCommands>,
    servers: BTreeMap<String, ServerReadings>,
}

#[derive(Debug, Clone)]
pub enum ScopedServersMessage {
    Services(SourceDelivery<ServiceCommands>),
    Switch(String, UnitSwitchMessage),
    Server(String, SourceDelivery<LlamaServer>),
}

impl ScopedLlamaServers {
    pub fn new(scope: SystemdUnitScope) -> Self {
        Self {
            scope,
            services: ContinuousReading::default(),
            servers: BTreeMap::new(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<ScopedServersMessage> {
        let systemd = SystemdOnBus {
            buses: buses.clone(),
            scope: self.scope,
        };
        Subscription::batch(
            [source_subscription::<ServiceCommands>(systemd).map(ScopedServersMessage::Services)]
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
                        source_subscription::<LlamaServer>(server)
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
                self.services.receive_keeping_last_reading(delivery, clock);
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
                    readings
                        .server
                        .receive_keeping_last_reading(delivery, clock);
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

    pub fn tooltip_for(
        &self,
        target: &LlamaServerTarget,
        model_groups: Vec<SourcedSections>,
        clock: Clock,
    ) -> Option<Tooltip> {
        let readings = self.servers.get(&target.unit)?;
        let reading = &readings.server;
        let server = reading.get();
        match target.reading {
            LlamaServerReading::Switch => readings.switch.tooltip(server.is_some(), clock),
            LlamaServerReading::Model => {
                let server = server?;
                let resident = server.resident_model();
                let endpoint = &readings.discovered.endpoint;
                let status_and_web_ui = Section::dictionary(
                    resident
                        .map(|model| KeyValue::new("Status", model.status))
                        .into_iter()
                        .chain([KeyValue::new(
                            "Web UI",
                            Cell::Link {
                                label: endpoint.clone(),
                                url: endpoint.clone(),
                                glyph: None,
                            },
                        )]),
                );
                let available_models = Section::dictionary(
                    server
                        .models
                        .iter()
                        .filter(|model| Some(*model) != resident)
                        .map(|model| KeyValue::new(model.id.clone(), model.status)),
                )
                .titled("Available models");
                Some(
                    reading
                        .tooltip_with(TooltipBody::sections([status_and_web_ui, available_models]))?
                        .with_groups(model_groups),
                )
            }
            LlamaServerReading::GenerationRate => {
                reading.tooltip_with(GenerationRate(server?.generation_rate?).tooltip(clock))
            }
            LlamaServerReading::Slots => {
                reading.tooltip_with(SlotOccupancy(&server?.slots).tooltip(clock))
            }
            LlamaServerReading::ContextFill => {
                reading.tooltip_with(ContextFill::of_busy_slots(&server?.slots)?.tooltip(clock))
            }
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
                .filter(|readings| readings.switch.running())
                .and_then(|readings| readings.server.get());
            let busy = running.filter(|server| server.busy_slots().next().is_some());
            [
                running.map(|server| {
                    let resident = server.resident_model();
                    GroupItem::reading(ServedModel { resident }, target(LlamaServerReading::Model))
                        .previewed_on_handle_if(resident.is_some())
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
                    readings.server.get().is_some(),
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
