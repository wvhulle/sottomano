pub mod core_kind_sections;
pub mod cpu_clock_label;
pub mod cpu_load_gauge;
pub mod cpu_temperature_label;
pub mod memory_usage_gauge;

use iced::{Subscription, Task};

use crate::{
    config::temperature::SensorConfig,
    data::source::{
        Source,
        hardware::HostHardware,
        sysfs_sensors::{
            self, cpu_clock::CpuClock, cpu_usage::CpuUsage, memory::Memory,
            temperature::Temperature,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        hardware_section::HardwareEntries,
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Default)]
pub struct ComputeGroup {
    cpu: LastPoll<CpuUsage>,
    memory: LastPoll<Memory>,
    clock: LastPoll<CpuClock>,
    temperature: LastPoll<Temperature>,
    sensors: bool,
    hardware: HostHardware,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ComputeMessage {
    Sensors(bool),
    Identified(HostHardware),
    Cpu(Result<CpuUsage, sysfs_sensors::Error>),
    Memory(Result<Memory, sysfs_sensors::Error>),
    Clock(Result<CpuClock, sysfs_sensors::Error>),
    Temperature(Result<Temperature, sysfs_sensors::Error>),
    Expansion(ExpansionMessage),
}

impl ComputeGroup {
    pub fn probe(sensor: &SensorConfig) -> Task<ComputeMessage> {
        let sensor = sensor.clone();
        Task::batch([
            Task::future(
                async move { ComputeMessage::Sensors(Temperature::available(&sensor).await) },
            ),
            Task::future(HostHardware::identify()).map(ComputeMessage::Identified),
        ])
    }

    pub fn subscription(&self, sensor: &SensorConfig) -> Subscription<ComputeMessage> {
        let temperature = if self.sensors {
            polling::<Temperature>(sensor.clone()).map(ComputeMessage::Temperature)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            polling::<CpuUsage>(()).map(ComputeMessage::Cpu),
            polling::<Memory>(()).map(ComputeMessage::Memory),
            polling::<CpuClock>(()).map(ComputeMessage::Clock),
            temperature,
        ])
    }

    pub fn update(&mut self, msg: ComputeMessage) -> Task<ComputeMessage> {
        match msg {
            ComputeMessage::Sensors(sensors) => self.sensors = sensors,
            ComputeMessage::Identified(hardware) => self.hardware = hardware,
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
                self.cpu.group_item(HoverTarget::Cpu),
                self.memory.group_item(HoverTarget::Memory),
                self.clock.group_item(HoverTarget::CpuClock),
                self.temperature.group_item(HoverTarget::Temperature),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Cpu => self
                .cpu
                .tooltip_with_sections(&self.hardware.cpu.hardware_sections()),
            HoverTarget::Memory => self
                .memory
                .tooltip_with_sections(&[self.hardware.memory.hardware_section()]),
            HoverTarget::CpuClock => self.clock.tooltip(),
            HoverTarget::Temperature => self.temperature.tooltip(),
            _ => None,
        }
    }
}
