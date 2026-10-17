use std::{collections::BTreeMap, mem::take};

use iced::{Subscription, Task};

use super::{
    LlamaServerReading, LlamaServerTarget, SERVER_SWITCH, context_fill_gauge::ContextFill,
    generation_rate_label::GenerationRate, loaded_model_label::ServedModel,
    slot_occupancy_gauge::SlotOccupancy,
};
use crate::{
    data::source::{
        Delivery,
        llama_server::{LlamaModel, LlamaServer, LlamaServerUnit, ModelInstance},
        systemd_manager::{UnitScope, service_commands::ServiceCommands},
    },
    message_subscriptions::listening,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        tooltip_body::{Section, Tooltip, TooltipBody},
        unit_switch::{UnitSwitch, UnitSwitchMessage},
    },
};

struct ServerReadings<S: UnitScope> {
    discovered: LlamaServerUnit<S>,
    switch: UnitSwitch<S>,
    server: ContinuousReading<LlamaServer<S>>,
}

impl<S: UnitScope> ServerReadings<S> {
    fn new(discovered: LlamaServerUnit<S>) -> Self {
        Self {
            switch: UnitSwitch::new(discovered.unit.clone(), SERVER_SWITCH),
            discovered,
            server: ContinuousReading::default(),
        }
    }
}

pub struct ScopedLlamaServers<S: UnitScope> {
    services: ContinuousReading<ServiceCommands<S>>,
    servers: BTreeMap<String, ServerReadings<S>>,
}

impl<S: UnitScope> Default for ScopedLlamaServers<S> {
    fn default() -> Self {
        Self {
            services: ContinuousReading::default(),
            servers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ScopedServersMessage<S: UnitScope> {
    Services(Delivery<ServiceCommands<S>>),
    Switch(String, UnitSwitchMessage<S>),
    Server(String, Delivery<LlamaServer<S>>),
}

impl<S: UnitScope> ScopedLlamaServers<S> {
    pub fn subscription(&self) -> Subscription<ScopedServersMessage<S>> {
        Subscription::batch(
            [listening::<ServiceCommands<S>>(()).map(ScopedServersMessage::Services)]
                .into_iter()
                .chain(self.servers.values().flat_map(|readings| {
                    let unit = readings.discovered.unit.clone();
                    [
                        readings
                            .switch
                            .subscription()
                            .with(unit.clone())
                            .map(|(unit, message)| ScopedServersMessage::Switch(unit, message)),
                        listening::<LlamaServer<S>>(readings.discovered.clone())
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
            .filter_map(LlamaServerUnit::discovered)
            .map(|discovered| {
                let readings = previous
                    .remove(&discovered.unit)
                    .filter(|readings| readings.discovered == discovered)
                    .unwrap_or_else(|| ServerReadings::new(discovered));
                (readings.discovered.unit.clone(), readings)
            })
            .collect();
    }

    pub fn update(&mut self, message: ScopedServersMessage<S>) -> Task<ScopedServersMessage<S>> {
        match message {
            ScopedServersMessage::Services(delivery) => {
                self.services.push_or_keep(delivery);
                self.adopt_discovered();
                Task::none()
            }
            ScopedServersMessage::Switch(unit, message) => match self.servers.get_mut(&unit) {
                Some(readings) => readings
                    .switch
                    .update(message)
                    .map(move |message| ScopedServersMessage::Switch(unit.clone(), message)),
                None => Task::none(),
            },
            ScopedServersMessage::Server(unit, delivery) => {
                if let Some(readings) = self.servers.get_mut(&unit) {
                    readings.server.push_or_keep(delivery);
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

    pub fn tooltip_for(
        &self,
        target: &LlamaServerTarget,
        model_props: Vec<Section>,
    ) -> Option<Tooltip> {
        let readings = self.servers.get(&target.unit)?;
        let reading = &readings.server;
        let server = reading.get();
        match target.reading {
            LlamaServerReading::Switch => readings.switch.tooltip(server.is_some()),
            LlamaServerReading::Model => Some(
                reading
                    .tooltip_with(TooltipBody::sections(
                        server?.model_sections(&readings.discovered.endpoint),
                    ))?
                    .with_sections(model_props),
            ),
            LlamaServerReading::GenerationRate => {
                reading.tooltip_with(GenerationRate(server?.generation_rate?).tooltip())
            }
            LlamaServerReading::Slots => {
                reading.tooltip_with(SlotOccupancy(&server?.slots).tooltip())
            }
            LlamaServerReading::ContextFill => {
                reading.tooltip_with(ContextFill::of_busy_slots(&server?.slots)?.tooltip())
            }
        }
    }

    pub fn resident_models(&self) -> impl Iterator<Item = &LlamaModel> {
        self.servers
            .values()
            .filter(|readings| readings.switch.running())
            .filter_map(|readings| readings.server.get()?.resident_model())
    }

    pub fn items<'a, M: 'a>(
        &'a self,
        lift: fn(ScopedServersMessage<S>) -> M,
    ) -> impl Iterator<Item = GroupItem<'a, M>> + 'a {
        self.servers.values().flat_map(move |readings| {
            let unit = &readings.discovered.unit;
            let target = |reading| LlamaServerTarget {
                bus: S::BUS,
                unit: unit.clone(),
                reading,
            };
            let running = Some(readings)
                .filter(|readings| readings.switch.running())
                .and_then(|readings| readings.server.get());
            let busy = running.filter(|server| server.busy_slots().next().is_some());
            [
                running.map(|server| {
                    GroupItem::reading(
                        ServedModel {
                            resident: server.resident_model(),
                        },
                        target(LlamaServerReading::Model),
                    )
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
