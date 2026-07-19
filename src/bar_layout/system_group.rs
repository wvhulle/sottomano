//! odin's `System` group: cpu handle expands to temperature/memory/disk. The only
//! expandable menu group today, so it owns its own open/hover state directly rather than
//! going through a cross-group menu coordinator (odin's `MenuGroup`/`GroupMenuState`).

use iced::widget::Row;
use iced::{Alignment, Element};

use crate::bar_program::Message;
use crate::data_sources::sysfs_sensors::{CpuUsage, Memory, Temperature};
use crate::data_sources::disk::Disk;
use crate::layer_surface::Position;
use crate::user_config::Config;
use crate::visual_elements::{as_bar_widget, edge_slot, hairline, leaf};

#[derive(Default)]
pub struct SystemGroup {
    cpu: Option<CpuUsage>,
    memory: Option<Memory>,
    temperature: Option<Temperature>,
    disk: Option<Disk>,
    /// Whether this group's content (temperature/memory/disk) is expanded.
    open: bool,
    /// Whether the pointer is over the handle, for the hairline's hover state.
    hovered: bool,
}

impl SystemGroup {
    pub fn set_cpu(&mut self, data: Option<CpuUsage>) {
        self.cpu = data;
    }

    pub fn set_memory(&mut self, data: Option<Memory>) {
        self.memory = data;
    }

    pub fn set_temperature(&mut self, data: Option<Temperature>) {
        self.temperature = data;
    }

    pub fn set_disk(&mut self, data: Option<Disk>) {
        self.disk = data;
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn hover(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// The handle, plus content when open, under a hairline that marks it expandable
    /// (faint at rest, brighter on hover, solid when open). The whole group toggles on
    /// click; hover drives the hairline. `None` if the handle (cpu) is unavailable.
    pub fn view<'a>(&self, config: &Config, position: Position) -> Option<Element<'a, Message>> {
        let handle = leaf(config, self.cpu.as_ref().map(as_bar_widget))?;

        let mut items = vec![handle];
        if self.open {
            items.extend([
                leaf(config, self.temperature.as_ref().map(as_bar_widget)),
                leaf(config, self.memory.as_ref().map(as_bar_widget)),
                leaf(config, self.disk.as_ref().map(as_bar_widget)),
            ].into_iter().flatten());
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
            config.palette.power_tint.0,
            opacity,
            config.layout.focus_underline_thickness,
        );

        Some(
            iced::widget::mouse_area(edge_slot(position, inner.into(), indicator, None))
                .on_press(Message::ToggleMenu)
                .on_enter(Message::HoverMenu(true))
                .on_exit(Message::HoverMenu(false))
                .into(),
        )
    }
}
