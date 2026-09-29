pub mod cpu_clock_label;
pub mod cpu_load_gauge;
pub mod cpu_temperature_gauge;
pub mod disk_usage_gauge;
pub mod memory_usage_gauge;

use iced::{Element, Subscription, Task, widget::Row};

use crate::{
    config::Config,
    data::source::{
        Source,
        disk::{self, Disk},
        sysfs_sensors::{self, cpu_usage::CpuUsage, memory::Memory, temperature::Temperature},
    },
    message_subscriptions::polling,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        active_reading_alert::{ActiveReadingAlert, highest_active_alert_severity},
        bar_widget::BarWidget,
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        source_reading::source_reading,
        tooltip_body::Tooltip,
    },
};

fn hover_reading<'a>(
    config: &Config,
    widget: &dyn BarWidget,
    opacity: f32,
    target: HoverTarget,
) -> Option<Element<'a, SystemMessage>> {
    let reading = source_reading(config, Some(widget), opacity)?;
    Some(hoverable(reading, target, SystemMessage::Tooltip))
}

#[derive(Default)]
pub struct SystemGroup {
    cpu: LastPoll<CpuUsage>,
    memory: LastPoll<Memory>,
    temperature: LastPoll<Temperature>,
    disk: LastPoll<Disk>,
    sensors: bool,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum SystemMessage {
    Sensors(bool),
    Cpu(Result<CpuUsage, sysfs_sensors::Error>),
    Memory(Result<Memory, sysfs_sensors::Error>),
    Temperature(Result<Temperature, sysfs_sensors::Error>),
    Disk(Result<Disk, disk::Error>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
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
            SystemMessage::Menu(m) => {
                self.menu.update(m);
            }
            SystemMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    pub fn active_reading_alerts(&self) -> Vec<ActiveReadingAlert<SystemMessage>> {
        let alert_expanding_group = |hover_target, widget: Option<&dyn BarWidget>| {
            ActiveReadingAlert::from_widget_state(
                hover_target,
                widget,
                SystemMessage::Menu(MenuMessage::ExpandGroup),
            )
        };
        [
            alert_expanding_group(
                HoverTarget::Temperature,
                self.temperature.get().map(|w| w as &dyn BarWidget),
            ),
            alert_expanding_group(
                HoverTarget::Memory,
                self.memory.get().map(|w| w as &dyn BarWidget),
            ),
            alert_expanding_group(
                HoverTarget::Disk,
                self.disk.get().map(|w| w as &dyn BarWidget),
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
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

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, SystemMessage>> {
        let cpu = self.cpu.get().map(|w| w as &dyn BarWidget)?;
        hover_reading(config, cpu, 1.0, HoverTarget::Cpu)?;

        let face = move || {
            hover_reading(config, cpu, 1.0, HoverTarget::Cpu).unwrap_or_else(|| Row::new().into())
        };

        let content = move |reveal: f32| {
            ReadingRow::new()
                .opacity(reveal)
                .push(Reading::new(
                    self.temperature.display(),
                    HoverTarget::Temperature,
                ))
                .push(Reading::new(self.memory.display(), HoverTarget::Memory))
                .push(Reading::new(self.disk.display(), HoverTarget::Disk))
                .row(config, SystemMessage::Tooltip)
                .unwrap_or_else(Row::new)
                .into()
        };

        Some(
            self.menu
                .slot(face, content, SystemMessage::Menu)
                .edge_indicator_alert_severity(highest_active_alert_severity(
                    &self.active_reading_alerts(),
                ))
                .view(config, position),
        )
    }
}
