mod board_firmware_face;
mod growth_trend_label;
mod memory_usage_gauge;
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
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::{Config, widget_groups::memory_growth_trend::MemoryGrowthTrendConfig};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, PanelVisibility},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBoardTarget {
    Memory,
    Growth,
    Pressure,
    Swap,
    Board,
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
    memory_processes: HoverFetchedSnapshot<MemoryProcesses>,
    swapped_processes: HoverFetchedSnapshot<SwappedProcesses>,
    module_temperatures: HoverFetchedSnapshot<MemoryModuleTemperatures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum MemoryBoardMessage {
    Hardware(Box<SnapshotDelivery<HostHardware>>),
    Memory(SnapshotDelivery<Memory>),
    MemoryGrowth(SnapshotDelivery<MemoryGrowth>),
    StallPressure(SnapshotDelivery<MemoryStallPressure>),
    SwapActivity(SnapshotDelivery<SwapActivity>),
    MemoryProcessesFetched(SnapshotDelivery<MemoryProcesses>),
    SwappedProcessesFetched(SnapshotDelivery<SwappedProcesses>),
    ModuleTemperaturesFetched(SnapshotDelivery<MemoryModuleTemperatures>),
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
        self.module_temperatures.stop();
        match hovered {
            Some(HoverTarget::MemoryBoard(MemoryBoardTarget::Memory)) => Task::batch([
                self.memory_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::MemoryProcessesFetched,
                    config,
                    clock,
                ),
                self.module_temperatures.start_hover_fetch(
                    (),
                    MemoryBoardMessage::ModuleTemperaturesFetched,
                    config,
                    clock,
                ),
            ]),
            Some(HoverTarget::MemoryBoard(MemoryBoardTarget::Swap)) => {
                self.swapped_processes.start_hover_fetch(
                    (),
                    MemoryBoardMessage::SwappedProcessesFetched,
                    config,
                    clock,
                )
            }
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
            MemoryBoardMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive(fetched, clock);
            }
            MemoryBoardMessage::SwappedProcessesFetched(fetched) => {
                self.swapped_processes.receive(fetched, clock);
            }
            MemoryBoardMessage::ModuleTemperaturesFetched(fetched) => {
                self.module_temperatures.receive(fetched, clock);
            }
            MemoryBoardMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, MemoryBoardMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::MemoryBoard,
            &self.expansion,
            MemoryBoardMessage::Expansion,
            [
                self.memory.group_item(MemoryBoardTarget::Memory),
                self.memory_growth.group_item(MemoryBoardTarget::Growth),
                self.stall_pressure.group_item(MemoryBoardTarget::Pressure),
                self.swap_activity.group_item(MemoryBoardTarget::Swap),
                self.hardware
                    .get()
                    .map(|hardware| &hardware.machine)
                    .filter(|reading| reading.panel_visibility() == PanelVisibility::Shown)
                    .map(|machine| GroupItem::reading(machine, MemoryBoardTarget::Board)),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn refresh(
        target: MemoryBoardTarget,
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
            MemoryBoardTarget::Board => buses.map_or_else(Task::none, |buses| {
                first_delivery::<HostHardware>(buses.clone())
                    .map(|delivery| MemoryBoardMessage::Hardware(Box::new(delivery)))
            }),
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: MemoryBoardTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            MemoryBoardTarget::Memory => self.memory.hover_card_with_groups(
                [
                    self.memory_processes.sections(clock),
                    self.module_temperatures.sections(clock),
                    self.hardware
                        .sections_described_by(|hardware| vec![Section::from(&hardware.memory)]),
                ]
                .into_iter()
                .flatten(),
                clock,
            ),
            MemoryBoardTarget::Growth => self.memory_growth.hover_card(clock),
            MemoryBoardTarget::Pressure => self.stall_pressure.hover_card(clock),
            MemoryBoardTarget::Swap => self
                .swap_activity
                .hover_card_with_groups(self.swapped_processes.sections(clock), clock),
            MemoryBoardTarget::Board => {
                let machine = &self.hardware.get()?.machine;
                self.hardware.hover_card_with(
                    (machine.panel_visibility() == PanelVisibility::Shown)
                        .then(|| machine.hover_card(clock))?,
                )
            }
        }
    }
}
