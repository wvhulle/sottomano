mod growth_trend_label;
mod memory_board_rows;
mod memory_slot_indicator;
mod memory_usage_gauge;
mod module_temperature_indicator;
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
    host_machine_identity::udev_hardware_identity::HostHardware,
};
use sottomano_toml_config::{
    Config, indicator_groups::memory_growth_trend::MemoryGrowthTrendConfig,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    indicator_groups::indicator_group_content::{
        IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBoardSource {
    Memory,
    Growth,
    Pressure,
    Swap,
    ModuleTemperatures,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryBoardInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    StartMemoryProcessesFetch,
    StartSwappedProcessesFetch,
    StopHoverFetches,
    Refresh(MemoryBoardSource),
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
    expansion: IndicatorGroupExpansion,
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
}

impl MemoryBoardGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
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
            MemoryBoardMessage::Memory(sample) => self.memory.receive(sample, clock),
            MemoryBoardMessage::MemoryGrowth(sample) => self.memory_growth.receive(sample, clock),
            MemoryBoardMessage::StallPressure(sample) => {
                self.stall_pressure.receive(sample, clock);
            }
            MemoryBoardMessage::SwapActivity(sample) => self.swap_activity.receive(sample, clock),
            MemoryBoardMessage::ModuleTemperatures(sample) => {
                self.module_temperatures.receive(sample, clock);
            }
            MemoryBoardMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive(fetched, clock);
            }
            MemoryBoardMessage::SwappedProcessesFetched(fetched) => {
                self.swapped_processes.receive(fetched, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: MemoryBoardInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<MemoryBoardMessage> {
        match interaction {
            MemoryBoardInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            MemoryBoardInteraction::StartMemoryProcessesFetch => {
                self.memory_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::MemoryProcessesFetched,
                    config,
                    clock,
                )
            }
            MemoryBoardInteraction::StartSwappedProcessesFetch => {
                self.swapped_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::SwappedProcessesFetched,
                    config,
                    clock,
                )
            }
            MemoryBoardInteraction::StopHoverFetches => {
                self.memory_processes.stop();
                self.swapped_processes.stop();
                Task::none()
            }
            MemoryBoardInteraction::Refresh(source) => {
                Self::refresh(source, &config.indicator_groups.memory_board.growth_trend)
            }
        }
    }

    fn refresh(
        source: MemoryBoardSource,
        growth_trend_config: &MemoryGrowthTrendConfig,
    ) -> Task<MemoryBoardMessage> {
        match source {
            MemoryBoardSource::Memory => {
                first_delivery::<Memory>(()).map(MemoryBoardMessage::Memory)
            }
            MemoryBoardSource::Growth => {
                first_delivery::<MemoryGrowth>(growth_trend_config.clone())
                    .map(MemoryBoardMessage::MemoryGrowth)
            }
            MemoryBoardSource::Pressure => {
                first_delivery::<MemoryStallPressure>(()).map(MemoryBoardMessage::StallPressure)
            }
            MemoryBoardSource::Swap => {
                first_delivery::<SwapActivity>(()).map(MemoryBoardMessage::SwapActivity)
            }
            MemoryBoardSource::ModuleTemperatures => first_delivery::<MemoryModuleTemperatures>(())
                .map(MemoryBoardMessage::ModuleTemperatures),
        }
    }
}
