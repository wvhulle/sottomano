pub mod core_kind_sections;
pub mod cpu_clock_label;
pub mod cpu_load_gauge;
pub mod cpu_temperature_label;

use iced::{Subscription, Task};

use crate::{
    data::source::{
        Source,
        sysfs_sensors::{self, cpu_clock::CpuClock, cpu_usage::CpuUsage, temperature::Temperature},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

#[derive(Default)]
pub struct ProcessorGroup {
    cpu: LastPoll<CpuUsage>,
    clock: LastPoll<CpuClock>,
    temperature: LastPoll<Temperature>,
    sensors: bool,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ProcessorMessage {
    Sensors(bool),
    Cpu(Result<CpuUsage, sysfs_sensors::Error>),
    Clock(Result<CpuClock, sysfs_sensors::Error>),
    Temperature(Result<Temperature, sysfs_sensors::Error>),
    Expansion(ExpansionMessage),
}

impl ProcessorGroup {
    pub fn probe() -> Task<ProcessorMessage> {
        Task::future(async { ProcessorMessage::Sensors(Temperature::available(&()).await) })
    }

    pub fn subscription(&self) -> Subscription<ProcessorMessage> {
        let temperature = if self.sensors {
            polling::<Temperature>(()).map(ProcessorMessage::Temperature)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            polling::<CpuUsage>(()).map(ProcessorMessage::Cpu),
            polling::<CpuClock>(()).map(ProcessorMessage::Clock),
            temperature,
        ])
    }

    pub fn update(&mut self, msg: ProcessorMessage) -> Task<ProcessorMessage> {
        match msg {
            ProcessorMessage::Sensors(sensors) => self.sensors = sensors,
            ProcessorMessage::Cpu(reading) => self.cpu.receive(reading),
            ProcessorMessage::Clock(reading) => self.clock.receive(reading),
            ProcessorMessage::Temperature(reading) => self.temperature.receive(reading),
            ProcessorMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn widget_group(&self) -> WidgetGroup<'_, ProcessorMessage> {
        WidgetGroup::new(&self.expansion, ProcessorMessage::Expansion)
            .reading(self.cpu.display(), HoverTarget::Cpu)
            .reading(self.clock.display(), HoverTarget::CpuClock)
            .reading(self.temperature.display(), HoverTarget::Temperature)
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Cpu => self.cpu.tooltip(),
            HoverTarget::CpuClock => self.clock.tooltip(),
            HoverTarget::Temperature => self.temperature.tooltip(),
            _ => None,
        }
    }
}
