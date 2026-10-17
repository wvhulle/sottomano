use std::collections::BTreeMap;

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
        tooltip_body::{SourcedSection, Tooltip, TooltipBody},
        unit_switch::{UnitSwitch, UnitSwitchMessage},
    },
};

struct ServerReadings<S: UnitScope> {
    switch: UnitSwitch<S>,
    server: ContinuousReading<LlamaServer<S>>,
}

impl<S: UnitScope> Default for ServerReadings<S> {
    fn default() -> Self {
        Self {
            switch: UnitSwitch::default(),
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
    fn discovered(&self) -> impl Iterator<Item = LlamaServerUnit<S>> + '_ {
        self.services
            .get()
            .into_iter()
            .flat_map(|services| services.services.iter())
            .filter_map(LlamaServerUnit::discovered)
    }

    pub fn subscription(&self) -> Subscription<ScopedServersMessage<S>> {
        Subscription::batch(
            [listening::<ServiceCommands<S>>(()).map(ScopedServersMessage::Services)]
                .into_iter()
                .chain(self.discovered().flat_map(|server| {
                    [
                        UnitSwitch::<S>::subscription(server.unit.clone())
                            .with(server.unit.clone())
                            .map(|(unit, message)| ScopedServersMessage::Switch(unit, message)),
                        listening::<LlamaServer<S>>(server.clone())
                            .with(server.unit.clone())
                            .map(|(unit, delivery)| ScopedServersMessage::Server(unit, delivery)),
                    ]
                })),
        )
    }

    pub fn update(&mut self, message: ScopedServersMessage<S>) -> Task<ScopedServersMessage<S>> {
        match message {
            ScopedServersMessage::Services(delivery) => {
                self.services.push_or_keep(delivery);
                Task::none()
            }
            ScopedServersMessage::Switch(unit, message) => self
                .servers
                .entry(unit.clone())
                .or_default()
                .switch
                .update(unit.clone(), message)
                .map(move |message| ScopedServersMessage::Switch(unit.clone(), message)),
            ScopedServersMessage::Server(unit, delivery) => {
                self.servers
                    .entry(unit)
                    .or_default()
                    .server
                    .push_or_keep(delivery);
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
        model_props: Vec<SourcedSection>,
    ) -> Option<Tooltip> {
        let readings = self.servers.get(&target.unit)?;
        let reading = &readings.server;
        let server = reading.get();
        match target.reading {
            LlamaServerReading::Switch => readings.switch.tooltip(SERVER_SWITCH, server.is_some()),
            LlamaServerReading::Model => {
                let discovered = self
                    .discovered()
                    .find(|discovered| discovered.unit == target.unit)?;
                Some(
                    reading
                        .tooltip_with(TooltipBody::sections(
                            server?.model_sections(&discovered.endpoint),
                        ))?
                        .with_sections(model_props),
                )
            }
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
        self.discovered().flat_map(move |discovered| {
            let readings = self.servers.get(&discovered.unit);
            let target = |reading| LlamaServerTarget {
                bus: S::BUS,
                unit: discovered.unit.clone(),
                reading,
            };
            let running = readings
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
                readings.and_then(|readings| {
                    readings.switch.group_item(
                        SERVER_SWITCH,
                        readings.server.get().is_some(),
                        target(LlamaServerReading::Switch),
                        lift(ScopedServersMessage::Switch(
                            discovered.unit.clone(),
                            UnitSwitchMessage::Press,
                        )),
                    )
                }),
            ]
            .into_iter()
            .flatten()
        })
    }
}
