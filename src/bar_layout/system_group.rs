//! Expandable system group: the cpu handle unfolds temperature/memory/disk.
//! A thin instance of the reusable menu-group primitive — it composes four
//! self-polling source cells and supplies a handle and a content row; the
//! expand animation lives in that primitive.

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::source_cell::{SourceCell, SourceMessage},
    data_sources::{
        BarWidget,
        disk::Disk,
        sysfs_sensors::{cpu_usage::CpuUsage, memory::Memory, temperature::Temperature},
    },
    layer_surface::Position,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    user_config::Config,
    widgets::{icon_gauge::source_reading, menu_group::MenuGroup},
};

/// A reading wrapped so hovering it opens its tooltip. Carries only the
/// `source` identity — the panel content is derived from the live reading at
/// render time, not snapshotted here.
fn hover_reading<'a>(
    config: &Config,
    widget: &dyn BarWidget,
    alpha: f32,
    source: TooltipSource,
) -> Option<Element<'a, SystemMessage>> {
    let reading = source_reading(config, Some(widget), alpha)?;
    Some(hoverable(reading, source, SystemMessage::Tooltip))
}

pub struct SystemGroup {
    cpu: SourceCell<CpuUsage>,
    memory: SourceCell<Memory>,
    temperature: SourceCell<Temperature>,
    disk: SourceCell<Disk>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum SystemMessage {
    Cpu(SourceMessage<CpuUsage>),
    Memory(SourceMessage<Memory>),
    Temperature(SourceMessage<Temperature>),
    Disk(SourceMessage<Disk>),
    Toggle,
    Hover(bool),
    Close,
    Tooltip(TooltipMessage),
}

impl SystemGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            cpu: SourceCell::default(),
            memory: SourceCell::default(),
            temperature: SourceCell::default(),
            disk: SourceCell::default(),
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn init() -> Task<SystemMessage> {
        Task::batch([
            SourceCell::init().map(SystemMessage::Cpu),
            SourceCell::init().map(SystemMessage::Memory),
            SourceCell::init().map(SystemMessage::Temperature),
            SourceCell::init().map(SystemMessage::Disk),
        ])
    }

    pub fn subscription() -> Subscription<SystemMessage> {
        Subscription::batch([
            SourceCell::subscription().map(SystemMessage::Cpu),
            SourceCell::subscription().map(SystemMessage::Memory),
            SourceCell::subscription().map(SystemMessage::Temperature),
            SourceCell::subscription().map(SystemMessage::Disk),
        ])
    }

    pub fn update(&mut self, msg: SystemMessage) -> Task<SystemMessage> {
        match msg {
            SystemMessage::Cpu(msg) => self.cpu.update(msg).map(SystemMessage::Cpu),
            SystemMessage::Memory(msg) => self.memory.update(msg).map(SystemMessage::Memory),
            SystemMessage::Temperature(msg) => {
                self.temperature.update(msg).map(SystemMessage::Temperature)
            }
            SystemMessage::Disk(msg) => self.disk.update(msg).map(SystemMessage::Disk),
            SystemMessage::Toggle => {
                self.menu.toggle();
                Task::none()
            }
            SystemMessage::Hover(hovered) => {
                self.menu.set_hover(hovered);
                Task::none()
            }
            SystemMessage::Close => {
                self.menu.close();
                Task::none()
            }
            SystemMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn cpu(&self) -> Option<&CpuUsage> {
        self.cpu.get()
    }

    pub fn memory(&self) -> Option<&Memory> {
        self.memory.get()
    }

    pub fn temperature(&self) -> Option<&Temperature> {
        self.temperature.get()
    }

    pub fn disk(&self) -> Option<&Disk> {
        self.disk.get()
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
            TooltipSource::Cpu,
        )?;

        let reveal = self.menu.reveal();
        let content = Row::with_children(
            [
                self.temperature
                    .get()
                    .map(|w| (w as &dyn BarWidget, TooltipSource::Temperature)),
                self.memory
                    .get()
                    .map(|w| (w as &dyn BarWidget, TooltipSource::Memory)),
                self.disk
                    .get()
                    .map(|w| (w as &dyn BarWidget, TooltipSource::Disk)),
            ]
            .into_iter()
            .flatten()
            .filter_map(|(widget, source)| hover_reading(config, widget, reveal, source)),
        )
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center);

        let group = self
            .menu
            .view(config, position, reveal, handle, content.into());

        Some(
            mouse_area(group)
                .on_press(SystemMessage::Toggle)
                .on_enter(SystemMessage::Hover(true))
                .on_exit(SystemMessage::Hover(false))
                .into(),
        )
    }
}
