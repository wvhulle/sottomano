use chrono::Locale;

use crate::widgets::icon_glyph::IconGlyph;
mod growth_trend_label;
mod memory_slot_reading;
mod memory_usage_gauge;
mod module_temperature_reading;
mod stall_pressure_label;
mod swap_activity_gauge;

use std::convert::identity;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        memory_growth_trend::MemoryGrowth,
        memory_module_temperatures::MemoryModuleTemperatures,
        memory_stall_pressure::MemoryStallPressure,
        memory_usage::{Memory, MemoryProcesses},
        swap_paging_activity::{SwapActivity, SwappedProcesses},
    },
    host_machine_identity::{
        hostname1_os_identity::MachineInfo, udev_hardware_identity::HostHardware,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, widget_groups::memory_growth_trend::MemoryGrowthTrendConfig};

use self::{
    growth_trend_label::{memory_growth_hover_card, memory_growth_reading_row},
    memory_slot_reading::MemorySlotReading,
    memory_usage_gauge::{memory_hover_card, memory_reading_row},
    module_temperature_reading::MemoryModuleTemperatureReading,
    stall_pressure_label::{memory_stall_hover_card, memory_stall_reading_row},
    swap_activity_gauge::{swap_activity_hover_card, swap_activity_reading_row},
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    hover_cards::HoverTarget,
    widget_groups::{
        memory_board_group::{
            memory_usage_gauge::memory_processes_sections,
            swap_activity_gauge::swapped_processes_sections,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryBoardTarget {
    Memory,
    Growth,
    Pressure,
    Swap,
    Slot(String),
    TemperatureSensor(String),
}

impl From<MemoryBoardTarget> for HoverTarget {
    fn from(target: MemoryBoardTarget) -> Self {
        HoverTarget::MemoryBoard(target)
    }
}

#[derive(Default)]
pub struct MemoryBoardGroup {
    memory: SubscribedSnapshot<Memory>,
    memory_growth: SubscribedSnapshot<MemoryGrowth>,
    stall_pressure: SubscribedSnapshot<MemoryStallPressure>,
    swap_activity: SubscribedSnapshot<SwapActivity>,
    hardware: SubscribedSnapshot<HostHardware>,
    module_temperatures: SubscribedSnapshot<MemoryModuleTemperatures>,
    memory_processes: HoverFetchedSnapshot<MemoryProcesses>,
    swapped_processes: HoverFetchedSnapshot<SwappedProcesses>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum MemoryBoardMessage {
    Hardware(Box<SnapshotDelivery<HostHardware>>),
    Memory(SnapshotDelivery<Memory>),
    MemoryGrowth(SnapshotDelivery<MemoryGrowth>),
    StallPressure(SnapshotDelivery<MemoryStallPressure>),
    SwapActivity(SnapshotDelivery<SwapActivity>),
    ModuleTemperatures(SnapshotDelivery<MemoryModuleTemperatures>),
    MemoryProcessesFetched(SnapshotDelivery<MemoryProcesses>),
    SwappedProcessesFetched(SnapshotDelivery<SwappedProcesses>),
    Expansion(WidgetGroupExpansionMessage),
}

impl MemoryBoardGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        clock: Clock,
    ) -> Task<MemoryBoardMessage> {
        self.memory_processes.stop();
        self.swapped_processes.stop();
        match hovered {
            Some(HoverTarget::MemoryBoard(target)) => match target {
                MemoryBoardTarget::Memory => self.memory_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::MemoryProcessesFetched,
                    config,
                    clock,
                ),
                MemoryBoardTarget::Swap => self.swapped_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::SwappedProcessesFetched,
                    config,
                    clock,
                ),
                _ => Task::none(),
            },
            _ => Task::none(),
        }
    }

    pub fn subscription(
        growth_trend_config: &MemoryGrowthTrendConfig,
    ) -> Subscription<MemoryBoardMessage> {
        Subscription::batch([
            snapshot_subscription::<Memory>(()).map(MemoryBoardMessage::Memory),
            snapshot_subscription::<MemoryGrowth>(growth_trend_config.clone())
                .map(MemoryBoardMessage::MemoryGrowth),
            snapshot_subscription::<MemoryStallPressure>(()).map(MemoryBoardMessage::StallPressure),
            snapshot_subscription::<SwapActivity>(()).map(MemoryBoardMessage::SwapActivity),
            snapshot_subscription::<MemoryModuleTemperatures>(())
                .map(MemoryBoardMessage::ModuleTemperatures),
        ])
    }

    pub fn update(
        &mut self,
        message: MemoryBoardMessage,
        clock: Clock,
    ) -> Task<MemoryBoardMessage> {
        match message {
            MemoryBoardMessage::Hardware(delivery) => {
                self.hardware.receive(*delivery, clock);
            }
            MemoryBoardMessage::Memory(reading) => self.memory.receive(reading, clock),
            MemoryBoardMessage::MemoryGrowth(reading) => self.memory_growth.receive(reading, clock),
            MemoryBoardMessage::StallPressure(reading) => {
                self.stall_pressure.receive(reading, clock);
            }
            MemoryBoardMessage::SwapActivity(reading) => self.swap_activity.receive(reading, clock),
            MemoryBoardMessage::ModuleTemperatures(reading) => {
                self.module_temperatures.receive(reading, clock);
            }
            MemoryBoardMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive(fetched, clock);
            }
            MemoryBoardMessage::SwappedProcessesFetched(fetched) => {
                self.swapped_processes.receive(fetched, clock);
            }
            MemoryBoardMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    fn slot_items(
        &self,
        locale: Locale,
    ) -> impl Iterator<Item = GroupItem<'_, WidgetGroupExpansionMessage>> {
        self.hardware.get().into_iter().flat_map(move |hardware| {
            let machine = &hardware.machine;
            hardware
                .memory
                .0
                .iter()
                .enumerate()
                .map(move |(slot_index, slot_module)| {
                    let slot_reading = MemorySlotReading {
                        slot_module: slot_module.clone(),
                        machine: if slot_index == 0 {
                            machine.clone()
                        } else {
                            MachineInfo::default()
                        },
                    };
                    GroupItem::reading(
                        slot_reading.clone().reading_row(locale),
                        MemoryBoardTarget::Slot(slot_module.locator.clone()),
                    )
                    .with_hover_card(Box::new(move |_| {
                        self.hardware.hover_card_with(slot_reading.hover_card())
                    }))
                })
        })
    }

    fn temperature_items(
        &self,
    ) -> impl Iterator<Item = GroupItem<'_, WidgetGroupExpansionMessage>> {
        self.module_temperatures
            .get()
            .into_iter()
            .flat_map(move |temperatures| {
                temperatures.modules.iter().map(move |module| {
                    let temperature_reading = MemoryModuleTemperatureReading {
                        bus_address: module.bus_address.clone(),
                        temperature: module.temperature,
                    };
                    GroupItem::reading(
                        temperature_reading.clone().reading_row(),
                        MemoryBoardTarget::TemperatureSensor(module.bus_address.clone()),
                    )
                    .with_hover_card(Box::new(move |_| {
                        self.module_temperatures
                            .hover_card_with(temperature_reading.hover_card())
                    }))
                })
            })
    }

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MemoryBoard,
            &self.expansion,
            identity,
            [
                self.memory
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| memory_reading_row(snapshot, locale),
                        MemoryBoardTarget::Memory,
                        clock,
                    )
                    .map(|memory_item| {
                        memory_item.with_hover_card(Box::new(move |_| {
                            let mut sections = self.memory.sections_described_by(memory_hover_card);
                            sections.extend(
                                self.memory_processes
                                    .sections_described_by(memory_processes_sections),
                            );
                            sections
                        }))
                    }),
                self.memory_growth
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| memory_growth_reading_row(snapshot, locale),
                        MemoryBoardTarget::Growth,
                        clock,
                    )
                    .map(|growth_item| {
                        growth_item.with_hover_card(Box::new(move |_| {
                            self.memory_growth
                                .sections_described_by(memory_growth_hover_card)
                        }))
                    }),
                self.stall_pressure
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| memory_stall_reading_row(snapshot, locale),
                        MemoryBoardTarget::Pressure,
                        clock,
                    )
                    .map(|pressure_item| {
                        pressure_item.with_hover_card(Box::new(move |_| {
                            self.stall_pressure
                                .sections_described_by(memory_stall_hover_card)
                        }))
                    }),
                self.swap_activity
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| swap_activity_reading_row(snapshot, locale),
                        MemoryBoardTarget::Swap,
                        clock,
                    )
                    .map(|swap_item| {
                        swap_item.with_hover_card(Box::new(move |_| {
                            let mut sections = self
                                .swap_activity
                                .sections_described_by(swap_activity_hover_card);
                            sections.extend(
                                self.swapped_processes
                                    .sections_described_by(swapped_processes_sections),
                            );
                            sections
                        }))
                    }),
            ]
            .into_iter()
            .flatten()
            .chain(self.slot_items(locale))
            .chain(self.temperature_items()),
        )
    }

    pub fn refresh(
        target: &MemoryBoardTarget,
        growth_trend_config: &MemoryGrowthTrendConfig,
        buses: Option<&DesktopBuses>,
    ) -> Task<MemoryBoardMessage> {
        match target {
            MemoryBoardTarget::Memory => {
                first_delivery::<Memory>(()).map(MemoryBoardMessage::Memory)
            }
            MemoryBoardTarget::Growth => {
                first_delivery::<MemoryGrowth>(growth_trend_config.clone())
                    .map(MemoryBoardMessage::MemoryGrowth)
            }
            MemoryBoardTarget::Pressure => {
                first_delivery::<MemoryStallPressure>(()).map(MemoryBoardMessage::StallPressure)
            }
            MemoryBoardTarget::Swap => {
                first_delivery::<SwapActivity>(()).map(MemoryBoardMessage::SwapActivity)
            }
            MemoryBoardTarget::Slot(_) => buses.map_or_else(Task::none, |buses| {
                first_delivery::<HostHardware>(buses.clone())
                    .map(|delivery| MemoryBoardMessage::Hardware(Box::new(delivery)))
            }),
            MemoryBoardTarget::TemperatureSensor(_) => {
                first_delivery::<MemoryModuleTemperatures>(())
                    .map(MemoryBoardMessage::ModuleTemperatures)
            }
        }
    }
}
