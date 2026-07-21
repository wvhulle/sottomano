//! Expandable system group: the cpu handle unfolds temperature/memory/disk.
//! A thin instance of the reusable [`MenuGroup`] — it owns only the readings
//! and supplies a handle and a content row; the expand animation lives in the
//! group.

use iced::{
    Alignment, Element, Task,
    time::Instant,
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
    widgets::{icon_gauge::source_reading, menu_group::MenuGroup},
};

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

    pub fn update(&mut self, msg: SystemMessage, now: Instant) -> Task<Message> {
        match msg {
            SystemMessage::LoadedCpu(data) => self.cpu = data,
            SystemMessage::LoadedMemory(data) => self.memory = data,
            SystemMessage::LoadedTemperature(data) => self.temperature = data,
            SystemMessage::LoadedDisk(data) => self.disk = data,
            SystemMessage::Toggle => self.menu.toggle(now),
            SystemMessage::Hover(hovered) => self.menu.set_hover(hovered),
            SystemMessage::Close => self.menu.close(now),
        }
        Task::none()
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
        now: Instant,
    ) -> Option<Element<'a, Message>> {
        let handle = source_reading(config, self.cpu.as_ref().map(|w| w as &dyn BarWidget), 1.0)?;

        let reveal = self.menu.reveal(now);
        let content = Row::with_children(
            [
                source_reading(
                    config,
                    self.temperature.as_ref().map(|w| w as &dyn BarWidget),
                    reveal,
                ),
                source_reading(
                    config,
                    self.memory.as_ref().map(|w| w as &dyn BarWidget),
                    reveal,
                ),
                source_reading(
                    config,
                    self.disk.as_ref().map(|w| w as &dyn BarWidget),
                    reveal,
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .spacing(config.layout.horizontal_widget_spacing)
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
