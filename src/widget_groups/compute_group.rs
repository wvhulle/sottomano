mod core_group_sections;
mod cpu_clock_label;
mod cpu_load_gauge;
mod cpu_temperature_label;
mod memory_usage_gauge;

use iced::{Subscription, Task, futures::Stream};

use crate::{
    config::{Config, widget_groups::temperature_sensors::SensorConfig},
    data::source::{
        Delivery, PolledSource,
        hardware::HostHardware,
        sysfs_sensors::{
            cpu_clock::CpuClock,
            cpu_usage::{CpuCoreLoads, CpuUsage},
            memory::{Memory, MemoryProcesses},
            temperature::{CpuCoreTemperatures, Temperature},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, holding_earliest, polling, polling_through},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        hardware_section::HardwareEntries,
        on_hover_reading::OnHoverReading,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeTarget {
    Cpu,
    CpuClock,
    Memory,
    Temperature,
    Machine,
}

impl From<ComputeTarget> for HoverTarget {
    fn from(target: ComputeTarget) -> Self {
        HoverTarget::Compute(target)
    }
}

#[derive(Default)]
pub struct ComputeGroup {
    cpu: ContinuousReading<CpuUsage>,
    memory: ContinuousReading<Memory>,
    clock: ContinuousReading<CpuClock>,
    temperature: ContinuousReading<Temperature>,
    hardware: ContinuousReading<HostHardware>,
    core_loads: OnHoverReading<CpuCoreLoads>,
    core_temperatures: OnHoverReading<CpuCoreTemperatures>,
    memory_processes: OnHoverReading<MemoryProcesses>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ComputeMessage {
    Hardware(Box<Delivery<HostHardware>>),
    Cpu(Delivery<CpuUsage>),
    Memory(Delivery<Memory>),
    Clock(Delivery<CpuClock>),
    Temperature(Delivery<Temperature>),
    CoreLoadsFetched(Delivery<CpuCoreLoads>),
    CoreTemperaturesFetched(Delivery<CpuCoreTemperatures>),
    MemoryProcessesFetched(Delivery<MemoryProcesses>),
    Expansion(ExpansionMessage),
}

fn confirmed_heat(sensor: SensorConfig) -> impl Stream<Item = Delivery<Temperature>> {
    holding_earliest(Temperature::polls(sensor), |reading| {
        &mut reading.critical_since
    })
}

impl ComputeGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<ComputeMessage> {
        self.core_loads.stop();
        self.core_temperatures.stop();
        self.memory_processes.stop();
        match hovered {
            Some(HoverTarget::Compute(ComputeTarget::Cpu)) => {
                self.core_loads
                    .poll((), ComputeMessage::CoreLoadsFetched, config)
            }
            Some(HoverTarget::Compute(ComputeTarget::Temperature)) => self.core_temperatures.poll(
                config.widget_groups.temperature_sensors.cpu.overrides(),
                ComputeMessage::CoreTemperaturesFetched,
                config,
            ),
            Some(HoverTarget::Compute(ComputeTarget::Memory)) => {
                self.memory_processes
                    .poll((), ComputeMessage::MemoryProcessesFetched, config)
            }
            _ => Task::none(),
        }
    }

    pub fn subscription(
        sensor: &SensorConfig,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<ComputeMessage> {
        Subscription::batch([
            polling::<HostHardware>((), forced)
                .map(|delivery| ComputeMessage::Hardware(Box::new(delivery))),
            polling::<CpuUsage>((), forced).map(ComputeMessage::Cpu),
            polling::<Memory>((), forced).map(ComputeMessage::Memory),
            polling::<CpuClock>((), forced).map(ComputeMessage::Clock),
            polling_through::<Temperature, _>(sensor.clone(), forced, confirmed_heat)
                .map(ComputeMessage::Temperature),
        ])
    }

    pub fn update(&mut self, msg: ComputeMessage) -> Task<ComputeMessage> {
        match msg {
            ComputeMessage::Hardware(delivery) => self.hardware.receive_or_keep(*delivery),
            ComputeMessage::CoreLoadsFetched(fetched) => self.core_loads.receive(fetched),
            ComputeMessage::CoreTemperaturesFetched(fetched) => {
                self.core_temperatures.receive(fetched);
            }
            ComputeMessage::MemoryProcessesFetched(fetched) => {
                self.memory_processes.receive(fetched);
            }
            ComputeMessage::Cpu(reading) => self.cpu.receive(reading),
            ComputeMessage::Memory(reading) => self.memory.receive(reading),
            ComputeMessage::Clock(reading) => self.clock.receive(reading),
            ComputeMessage::Temperature(reading) => self.temperature.receive(reading),
            ComputeMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, ComputeMessage> {
        GroupContent::new(
            &self.expansion,
            ComputeMessage::Expansion,
            [
                self.cpu.group_item(ComputeTarget::Cpu),
                self.memory.group_item(ComputeTarget::Memory),
                self.clock.group_item(ComputeTarget::CpuClock),
                self.temperature.group_item(ComputeTarget::Temperature),
                self.hardware
                    .get()
                    .map(|hardware| GroupItem::reading(&hardware.machine, ComputeTarget::Machine)),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn tooltip_for(&self, target: ComputeTarget) -> Option<Tooltip> {
        match target {
            ComputeTarget::Cpu => self
                .cpu
                .tooltip_with_sections(self.core_loads.sections(CpuCoreLoads::sections)),
            ComputeTarget::Memory => self.memory.tooltip_with_sections(
                [
                    self.memory_processes.sections(MemoryProcesses::sections),
                    self.hardware
                        .sections(|hardware| vec![hardware.memory.hardware_section()]),
                ]
                .concat(),
            ),
            ComputeTarget::CpuClock => self.clock.tooltip_with_sections(
                [
                    self.clock.sections(CpuClock::core_sections),
                    self.hardware
                        .sections(|hardware| hardware.cpu.hardware_sections().to_vec()),
                ]
                .concat(),
            ),
            ComputeTarget::Temperature => self.temperature.tooltip_with_sections(
                self.core_temperatures
                    .sections(CpuCoreTemperatures::sections),
            ),
            ComputeTarget::Machine => self
                .hardware
                .tooltip_with(self.hardware.get()?.machine.tooltip()),
        }
    }
}
