mod growth_trend_label;
mod memory_slot_reading;
mod memory_usage_gauge;
mod module_temperature_reading;
mod stall_pressure_label;
mod swap_activity_gauge;

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
    memory_slot_reading::MemorySlotReading,
    module_temperature_reading::MemoryModuleTemperatureReading,
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
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
    memory: LatestSnapshot<Memory>,
    memory_growth: LatestSnapshot<MemoryGrowth>,
    stall_pressure: LatestSnapshot<MemoryStallPressure>,
    swap_activity: LatestSnapshot<SwapActivity>,
    hardware: LatestSnapshot<HostHardware, HidesFetchFailures>,
    module_temperatures: LatestSnapshot<MemoryModuleTemperatures, HidesFetchFailures>,
    memory_processes: HoverFetchedSnapshot<MemoryProcesses>,
    swapped_processes: HoverFetchedSnapshot<SwappedProcesses>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
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

    pub fn update(&mut self, msg: MemoryBoardMessage, clock: Clock) -> Task<MemoryBoardMessage> {
        match msg {
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
            MemoryBoardMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, MemoryBoardMessage> {
        let slot_items = self.hardware.get().into_iter().flat_map(|hardware| {
            let machine = &hardware.machine;
            hardware
                .memory
                .0
                .iter()
                .enumerate()
                .map(|(slot_index, slot_module)| {
                    GroupItem::reading(
                        MemorySlotReading {
                            slot_module: slot_module.clone(),
                            machine: if slot_index == 0 {
                                machine.clone()
                            } else {
                                MachineInfo::default()
                            },
                        },
                        MemoryBoardTarget::Slot(slot_module.locator.clone()),
                    )
                })
        });
        let temperature_items =
            self.module_temperatures
                .get()
                .into_iter()
                .flat_map(|temperatures| {
                    temperatures.modules.iter().map(|module| {
                        GroupItem::reading(
                            MemoryModuleTemperatureReading {
                                bus_address: module.bus_address.clone(),
                                temperature: module.temperature,
                            },
                            MemoryBoardTarget::TemperatureSensor(module.bus_address.clone()),
                        )
                    })
                });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MemoryBoard,
            &self.expansion,
            MemoryBoardMessage::Expansion,
            [
                self.memory.group_item(MemoryBoardTarget::Memory),
                self.memory_growth.group_item(MemoryBoardTarget::Growth),
                self.stall_pressure.group_item(MemoryBoardTarget::Pressure),
                self.swap_activity.group_item(MemoryBoardTarget::Swap),
            ]
            .into_iter()
            .flatten()
            .chain(slot_items)
            .chain(temperature_items),
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

    #[must_use]
    pub fn hover_card_for(&self, target: &MemoryBoardTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            MemoryBoardTarget::Memory => self.memory.hover_card_with_groups(
                [self.memory_processes.sections(clock)]
                    .into_iter()
                    .flatten(),
                clock,
            ),
            MemoryBoardTarget::Growth => self.memory_growth.hover_card(clock),
            MemoryBoardTarget::Pressure => self.stall_pressure.hover_card(clock),
            MemoryBoardTarget::Swap => self
                .swap_activity
                .hover_card_with_groups(self.swapped_processes.sections(clock), clock),
            MemoryBoardTarget::Slot(locator) => {
                let hardware = self.hardware.get()?;
                let slot_module = hardware
                    .memory
                    .0
                    .iter()
                    .find(|slot_module| slot_module.locator == *locator)?;
                let machine = if slot_module.locator == hardware.memory.0[0].locator {
                    hardware.machine.clone()
                } else {
                    MachineInfo::default()
                };
                let reading = MemorySlotReading {
                    slot_module: slot_module.clone(),
                    machine,
                };
                self.hardware.hover_card_with(reading.hover_card(clock))
            }
            MemoryBoardTarget::TemperatureSensor(bus_address) => {
                let module = self
                    .module_temperatures
                    .get()?
                    .modules
                    .iter()
                    .find(|module| &module.bus_address == bus_address)?;
                let reading = MemoryModuleTemperatureReading {
                    bus_address: module.bus_address.clone(),
                    temperature: module.temperature,
                };
                self.module_temperatures
                    .hover_card_with(reading.hover_card(clock))
            }
        }
    }
}
