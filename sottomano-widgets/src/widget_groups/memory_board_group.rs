mod board_firmware_face;
mod memory_usage_gauge;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        memory_module_temperatures::MemoryModuleTemperatures,
        memory_usage::{Memory, MemoryProcesses},
    },
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
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
    hardware: LatestSnapshot<HostHardware, HidesFetchFailures>,
    memory_processes: HoverFetchedSnapshot<MemoryProcesses>,
    module_temperatures: HoverFetchedSnapshot<MemoryModuleTemperatures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum MemoryBoardMessage {
    Hardware(Box<SnapshotDelivery<HostHardware>>),
    Memory(SnapshotDelivery<Memory>),
    MemoryProcessesFetched(SnapshotDelivery<MemoryProcesses>),
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
            _ => Task::none(),
        }
    }

    pub fn subscription() -> Subscription<MemoryBoardMessage> {
        snapshot_subscription::<Memory>(()).map(MemoryBoardMessage::Memory)
    }

    pub fn update(&mut self, msg: MemoryBoardMessage, clock: Clock) -> Task<MemoryBoardMessage> {
        match msg {
            MemoryBoardMessage::Hardware(delivery) => {
                self.hardware.receive(*delivery, clock);
            }
            MemoryBoardMessage::Memory(reading) => self.memory.receive(reading, clock),
            MemoryBoardMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive(fetched, clock);
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
                self.hardware
                    .get()
                    .map(|hardware| &hardware.machine)
                    .filter(GroupItemReading::visible_on_panel)
                    .map(|machine| GroupItem::reading(machine, MemoryBoardTarget::Board)),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn refresh(
        target: MemoryBoardTarget,
        buses: Option<&DesktopBuses>,
    ) -> Task<MemoryBoardMessage> {
        match target {
            MemoryBoardTarget::Memory => {
                first_delivery::<Memory>(()).map(MemoryBoardMessage::Memory)
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
            MemoryBoardTarget::Board => {
                let machine = &self.hardware.get()?.machine;
                self.hardware.hover_card_with(
                    machine
                        .visible_on_panel()
                        .then(|| machine.hover_card(clock))?,
                )
            }
        }
    }
}
