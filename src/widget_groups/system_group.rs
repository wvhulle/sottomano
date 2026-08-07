use iced::{
    Element, Subscription,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::source::{
        BarWidget, Tooltip,
        disk::Disk,
        sysfs_sensors::{cpu_usage::CpuUsage, memory::Memory, temperature::Temperature},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::{pills::Pills, source_cell::SourceCell},
    widgets::{
        flat_button,
        icon_gauge::source_reading,
        menu_group::{MenuGroup, MenuMessage},
    },
};

fn hover_reading<'a>(
    config: &Config,
    widget: &dyn BarWidget,
    alpha: f32,
    target: HoverTarget,
) -> Option<Element<'a, SystemMessage>> {
    let reading = source_reading(config, Some(widget), alpha)?;
    Some(hoverable(reading, target, SystemMessage::Tooltip))
}

#[derive(Default)]
pub struct SystemGroup {
    cpu: SourceCell<CpuUsage>,
    memory: SourceCell<Memory>,
    temperature: SourceCell<Temperature>,
    disk: SourceCell<Disk>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum SystemMessage {
    Cpu(Option<CpuUsage>),
    Memory(Option<Memory>),
    Temperature(Option<Temperature>),
    Disk(Option<Disk>),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl SystemGroup {
    pub fn subscription() -> Subscription<SystemMessage> {
        Subscription::batch([
            SourceCell::subscription().map(SystemMessage::Cpu),
            SourceCell::subscription().map(SystemMessage::Memory),
            SourceCell::subscription().map(SystemMessage::Temperature),
            SourceCell::subscription().map(SystemMessage::Disk),
        ])
    }

    pub fn update(&mut self, msg: SystemMessage) {
        match msg {
            SystemMessage::Cpu(reading) => self.cpu.load(reading),
            SystemMessage::Memory(reading) => self.memory.load(reading),
            SystemMessage::Temperature(reading) => self.temperature.load(reading),
            SystemMessage::Disk(reading) => self.disk.load(reading),
            SystemMessage::Menu(m) => {
                self.menu.update(m);
            }
            SystemMessage::Tooltip(_) => {}
        }
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
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, SystemMessage>> {
        let handle = hover_reading(
            config,
            self.cpu.get().map(|w| w as &dyn BarWidget)?,
            1.0,
            HoverTarget::Cpu,
        )?;
        let handle = button(handle)
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(SystemMessage::Menu(MenuMessage::Toggle))
            .into();

        let reveal = self.menu.reveal();
        let content = Pills::new()
            .alpha(reveal)
            .push(self.temperature.get(), HoverTarget::Temperature)
            .push(self.memory.get(), HoverTarget::Memory)
            .push(self.disk.get(), HoverTarget::Disk)
            .row(config, SystemMessage::Tooltip)
            .unwrap_or_else(Row::new);

        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            SystemMessage::Menu,
        ))
    }
}
