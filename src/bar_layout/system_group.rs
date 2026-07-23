//! Expandable system group: the cpu handle unfolds temperature/memory/disk.
//! A thin instance of the reusable menu-group primitive — it owns only the
//! readings and supplies a handle and a content row; the expand animation
//! lives in that primitive.

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
    tooltip::hoverable,
    user_config::Config,
    widgets::{icon_gauge::source_reading, menu_group::MenuGroup},
};

/// A reading wrapped so hovering it opens its tooltip.
fn hover_reading<'a>(
    config: &Config,
    widget: &dyn BarWidget,
    alpha: f32,
) -> Option<Element<'a, Message>> {
    let reading = source_reading(config, Some(widget), alpha)?;
    Some(hoverable(reading, widget.tooltip(), Message::Tooltip))
}

pub struct SystemGroup {
    cpu: Option<CpuUsage>,
    memory: Option<Memory>,
    temperature: Option<Temperature>,
    disk: Option<Disk>,
    pub(crate) menu: MenuGroup,
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
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            cpu: None,
            memory: None,
            temperature: None,
            disk: None,
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn update(&mut self, msg: SystemMessage) -> Task<Message> {
        match msg {
            SystemMessage::LoadedCpu(data) => self.cpu = data,
            SystemMessage::LoadedMemory(data) => self.memory = data,
            SystemMessage::LoadedTemperature(data) => self.temperature = data,
            SystemMessage::LoadedDisk(data) => self.disk = data,
            SystemMessage::Toggle => self.menu.toggle(),
            SystemMessage::Hover(hovered) => self.menu.set_hover(hovered),
            SystemMessage::Close => self.menu.close(),
        }
        Task::none()
    }

    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, Message>> {
        let handle = hover_reading(config, self.cpu.as_ref().map(|w| w as &dyn BarWidget)?, 1.0)?;

        let reveal = self.menu.reveal();
        let content = Row::with_children(
            [
                self.temperature.as_ref().map(|w| w as &dyn BarWidget),
                self.memory.as_ref().map(|w| w as &dyn BarWidget),
                self.disk.as_ref().map(|w| w as &dyn BarWidget),
            ]
            .into_iter()
            .flatten()
            .filter_map(|widget| hover_reading(config, widget, reveal)),
        )
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center);

        let group = self
            .menu
            .view(config, position, reveal, handle, content.into());

        Some(
            mouse_area(group)
                .on_press(Message::System(SystemMessage::Toggle))
                .on_enter(Message::System(SystemMessage::Hover(true)))
                .on_exit(Message::System(SystemMessage::Hover(false)))
                .into(),
        )
    }
}
