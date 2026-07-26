//! Expandable system group: the cpu handle unfolds temperature/memory/disk.
//! A thin instance of the reusable menu-group primitive — it composes four
//! self-polling source cells and supplies a handle and a content row; the
//! expand animation lives in that primitive.

use iced::{
    Alignment, Element, Subscription,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget, Tooltip,
        disk::Disk,
        sysfs_sensors::{cpu_usage::CpuUsage, memory::Memory, temperature::Temperature},
    },
    layer_surface::Position,
    tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{
        icon_gauge::source_reading,
        menu_group::{MenuGroup, MenuMessage},
    },
};

/// A reading wrapped so hovering it opens its tooltip. Carries only the
/// `target` identity — the panel content is derived from the live reading at
/// render time, not snapshotted here.
fn hover_reading<'a>(
    config: &ResolvedConfig,
    widget: &dyn BarWidget,
    alpha: f32,
    target: HoverTarget,
) -> Option<Element<'a, SystemMessage>> {
    let reading = source_reading(config, Some(widget), alpha)?;
    Some(hoverable(reading, target, SystemMessage::Tooltip))
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
    Cpu(Option<CpuUsage>),
    Memory(Option<Memory>),
    Temperature(Option<Temperature>),
    Disk(Option<Disk>),
    Menu(MenuMessage),
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
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, SystemMessage>> {
        let handle = hover_reading(
            config,
            self.cpu.get().map(|w| w as &dyn BarWidget)?,
            1.0,
            HoverTarget::Cpu,
        )?;
        // The handle carries the toggle press (only the handle toggles, so
        // clicking the revealed readings never collapses the group).
        let handle = mouse_area(handle)
            .on_press(SystemMessage::Menu(MenuMessage::Toggle))
            .into();

        let reveal = self.menu.reveal();
        let content = Row::with_children(
            [
                self.temperature
                    .get()
                    .map(|w| (w as &dyn BarWidget, HoverTarget::Temperature)),
                self.memory
                    .get()
                    .map(|w| (w as &dyn BarWidget, HoverTarget::Memory)),
                self.disk
                    .get()
                    .map(|w| (w as &dyn BarWidget, HoverTarget::Disk)),
            ]
            .into_iter()
            .flatten()
            .filter_map(|(widget, target)| hover_reading(config, widget, reveal, target)),
        )
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center);

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
