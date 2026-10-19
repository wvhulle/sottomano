mod board_firmware_face;
mod memory_usage_gauge;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    cpu_memory_compute::{
        memory_module_temperatures::MemoryModuleTemperatures,
        memory_usage::{Memory, MemoryProcesses},
    },
    host_machine_identity::udev_hardware_identity::HostHardware,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        tooltip_body::{Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
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
    memory: ContinuousReading<Memory>,
    hardware: ContinuousReading<HostHardware>,
    memory_processes: OnHoverReading<MemoryProcesses>,
    module_temperatures: OnHoverReading<MemoryModuleTemperatures>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum MemoryBoardMessage {
    Hardware(Box<SourceDelivery<HostHardware>>),
    Memory(SourceDelivery<Memory>),
    MemoryProcessesFetched(SourceDelivery<MemoryProcesses>),
    ModuleTemperaturesFetched(SourceDelivery<MemoryModuleTemperatures>),
    Expansion(ExpansionMessage),
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
        source_subscription::<Memory>(()).map(MemoryBoardMessage::Memory)
    }

    pub fn update(&mut self, msg: MemoryBoardMessage, clock: Clock) -> Task<MemoryBoardMessage> {
        match msg {
            MemoryBoardMessage::Hardware(delivery) => {
                self.hardware.receive_keeping_last_reading(*delivery, clock);
            }
            MemoryBoardMessage::Memory(reading) => self.memory.receive_delivery(reading, clock),
            MemoryBoardMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive_delivery(fetched, clock);
            }
            MemoryBoardMessage::ModuleTemperaturesFetched(fetched) => {
                self.module_temperatures.receive_delivery(fetched, clock);
            }
            MemoryBoardMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, MemoryBoardMessage> {
        GroupContent::new(
            &self.expansion,
            MemoryBoardMessage::Expansion,
            [
                self.memory.group_item(MemoryBoardTarget::Memory),
                self.hardware
                    .get()
                    .map(|hardware| &hardware.machine)
                    .filter(BarWidget::visible_in_bar)
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
    pub fn tooltip_for(&self, target: MemoryBoardTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            MemoryBoardTarget::Memory => self.memory.tooltip_with_sections(
                [
                    self.memory_processes.sections(clock),
                    self.module_temperatures.sections(clock),
                    self.hardware
                        .sections_described_by(|hardware| vec![Section::from(&hardware.memory)]),
                ]
                .concat(),
                clock,
            ),
            MemoryBoardTarget::Board => {
                let machine = &self.hardware.get()?.machine;
                self.hardware
                    .tooltip_with(machine.visible_in_bar().then(|| machine.tooltip(clock))?)
            }
        }
    }
}
