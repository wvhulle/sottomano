//! Expandable system group: cpu handle expands to temperature/memory/disk.

use iced::{
    Alignment, Element, Task,
    widget::{Row, mouse_area},
};

use crate::{
    bar_program::Message,
    data_sources::{
        BarWidget,
        disk::Disk,
        sysfs_sensors::{cpu_usage::CpuUsage, memory::Memory, temperature::Temperature},
    },
    layer_surface::Position,
    user_config::Config,
    widgets::{activity_line::activity_line, icon_gauge::source_reading, pager_slot::pager_slot},
};

#[derive(Default)]
pub struct SystemGroup {
    cpu: Option<CpuUsage>,
    memory: Option<Memory>,
    temperature: Option<Temperature>,
    disk: Option<Disk>,
    open: bool,
    hovered: bool,
}

#[derive(Debug, Clone)]
pub enum SystemMessage {
    LoadedCpu(Option<CpuUsage>),
    LoadedMemory(Option<Memory>),
    LoadedTemperature(Option<Temperature>),
    LoadedDisk(Option<Disk>),
    Toggle,
    Hover(bool),
    Close,
}

impl SystemGroup {
    pub fn update(&mut self, msg: SystemMessage) -> Task<Message> {
        match msg {
            SystemMessage::LoadedCpu(data) => self.cpu = data,
            SystemMessage::LoadedMemory(data) => self.memory = data,
            SystemMessage::LoadedTemperature(data) => self.temperature = data,
            SystemMessage::LoadedDisk(data) => self.disk = data,
            SystemMessage::Toggle => self.open = !self.open,
            SystemMessage::Hover(hovered) => self.hovered = hovered,
            SystemMessage::Close => self.open = false,
        }
        Task::none()
    }

    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, Message>> {
        let handle = source_reading(config, self.cpu.as_ref().map(|w| w as &dyn BarWidget))?;

        let mut items = vec![handle];
        if self.open {
            items.extend(
                [
                    source_reading(
                        config,
                        self.temperature.as_ref().map(|w| w as &dyn BarWidget),
                    ),
                    source_reading(config, self.memory.as_ref().map(|w| w as &dyn BarWidget)),
                    source_reading(config, self.disk.as_ref().map(|w| w as &dyn BarWidget)),
                ]
                .into_iter()
                .flatten(),
            );
        }
        let inner = Row::with_children(items)
            .spacing(config.layout.horizontal_widget_spacing)
            .align_y(Alignment::Center);

        let opacity = if self.open {
            1.0
        } else if self.hovered {
            0.5
        } else {
            0.2
        };
        let indicator = activity_line(
            config.palette.focus(),
            opacity,
            config.layout.focus_underline_thickness,
        );

        Some(
            mouse_area(pager_slot(position, inner.into(), indicator, None))
                .on_press(Message::System(SystemMessage::Toggle))
                .on_enter(Message::System(SystemMessage::Hover(true)))
                .on_exit(Message::System(SystemMessage::Hover(false)))
                .into(),
        )
    }
}
