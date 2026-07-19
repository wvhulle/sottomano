//! odin's `System` group: cpu handle expands to temperature/memory/disk. The
//! only expandable menu group today, so it owns its own open/hover state
//! directly rather than going through a cross-group menu coordinator (odin's
//! `MenuGroup`/`GroupMenuState`).

use iced::{Alignment, Element, Task, widget::Row};

use crate::{
    bar_program::Message,
    data_sources::{
        BarWidget,
        disk::Disk,
        sysfs_sensors::{CpuUsage, Memory, Temperature},
    },
    layer_surface::Position,
    user_config::Config,
    visual_elements::{edge_slot, hairline, leaf},
};

/// The expandable cpu/temperature/memory/disk group, with its own hover/open
/// state.
#[derive(Default)]
pub struct SystemGroup {
    cpu: Option<CpuUsage>,
    memory: Option<Memory>,
    temperature: Option<Temperature>,
    disk: Option<Disk>,
    open: bool,
    hovered: bool,
}

/// Messages handled by [`SystemGroup::update`].
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
    /// Apply a [`SystemMessage`].
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

    /// The handle, plus content when open, under a hairline that marks it
    /// expandable (faint at rest, brighter on hover, solid when open). The
    /// whole group toggles on click; hover drives the hairline. `None` if
    /// the handle (cpu) is unavailable.
    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, Message>> {
        let handle = leaf(config, self.cpu.as_ref().map(|w| w as &dyn BarWidget))?;

        let mut items = vec![handle];
        if self.open {
            items.extend(
                [
                    leaf(
                        config,
                        self.temperature.as_ref().map(|w| w as &dyn BarWidget),
                    ),
                    leaf(config, self.memory.as_ref().map(|w| w as &dyn BarWidget)),
                    leaf(config, self.disk.as_ref().map(|w| w as &dyn BarWidget)),
                ]
                .into_iter()
                .flatten(),
            );
        }
        let inner = Row::with_children(items)
            .spacing(config.layout.horizontal_widget_spacing as f32)
            .align_y(Alignment::Center);

        let opacity = if self.open {
            1.0
        } else if self.hovered {
            0.5
        } else {
            0.2
        };
        let indicator = hairline(
            config.palette.focus(),
            opacity,
            config.layout.focus_underline_thickness,
        );

        Some(
            iced::widget::mouse_area(edge_slot(position, inner.into(), indicator, None))
                .on_press(Message::System(SystemMessage::Toggle))
                .on_enter(Message::System(SystemMessage::Hover(true)))
                .on_exit(Message::System(SystemMessage::Hover(false)))
                .into(),
        )
    }
}
