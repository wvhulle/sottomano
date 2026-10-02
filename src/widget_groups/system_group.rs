pub mod cpu_clock_label;
pub mod cpu_load_gauge;
pub mod cpu_temperature_gauge;
pub mod disk_usage_gauge;
pub mod memory_usage_gauge;

use iced::{Subscription, Task};

use crate::{
    data::source::{
        Source,
        disk::{self, Disk},
        sysfs_sensors::{self, cpu_usage::CpuUsage, memory::Memory, temperature::Temperature},
    },
    message_subscriptions::polling,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

#[derive(Default)]
pub struct SystemGroup {
    cpu: LastPoll<CpuUsage>,
    memory: LastPoll<Memory>,
    temperature: LastPoll<Temperature>,
    disk: LastPoll<Disk>,
    sensors: bool,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum SystemMessage {
    Sensors(bool),
    Cpu(Result<CpuUsage, sysfs_sensors::Error>),
    Memory(Result<Memory, sysfs_sensors::Error>),
    Temperature(Result<Temperature, sysfs_sensors::Error>),
    Disk(Result<Disk, disk::Error>),
    Expansion(ExpansionMessage),
}

impl SystemGroup {
    pub fn probe() -> Task<SystemMessage> {
        Task::future(async { SystemMessage::Sensors(Temperature::available(&()).await) })
    }

    pub fn subscription(&self) -> Subscription<SystemMessage> {
        let temperature = if self.sensors {
            polling::<Temperature>(()).map(SystemMessage::Temperature)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            polling::<CpuUsage>(()).map(SystemMessage::Cpu),
            polling::<Memory>(()).map(SystemMessage::Memory),
            temperature,
            polling::<Disk>(()).map(SystemMessage::Disk),
        ])
    }

    pub fn update(&mut self, msg: SystemMessage) -> Task<SystemMessage> {
        match msg {
            SystemMessage::Sensors(sensors) => self.sensors = sensors,
            SystemMessage::Cpu(reading) => self.cpu.receive(reading),
            SystemMessage::Memory(reading) => self.memory.receive(reading),
            SystemMessage::Temperature(reading) => self.temperature.receive(reading),
            SystemMessage::Disk(reading) => self.disk.receive(reading),
            SystemMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn widget_group(&self) -> WidgetGroup<'_, SystemMessage> {
        WidgetGroup::new(&self.expansion, SystemMessage::Expansion)
            .reading(self.cpu.display(), HoverTarget::Cpu)
            .reading(self.temperature.display(), HoverTarget::Temperature)
            .reading(self.memory.display(), HoverTarget::Memory)
            .reading(self.disk.display(), HoverTarget::Disk)
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Cpu => self.cpu.tooltip(),
            HoverTarget::Memory => self.memory.tooltip(),
            HoverTarget::Temperature => self.temperature.tooltip(),
            HoverTarget::Disk => self.disk.tooltip(),
            _ => None,
        }
    }
}
