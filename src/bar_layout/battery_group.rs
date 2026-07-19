//! odin's `WidgetGroup/Battery.qml`: a single-widget static group. Owns its own
//! polled reading.

use iced::{Element, Task};

use crate::{
    bar_program::Message,
    data_sources::{BarWidget, sysfs_sensors::Battery},
    user_config::Config,
    visual_elements::leaf,
};

/// A single-widget static group holding the battery reading.
#[derive(Default)]
pub struct BatteryGroup {
    battery: Option<Battery>,
}

/// Messages handled by [`BatteryGroup::update`].
#[derive(Debug, Clone)]
pub enum BatteryMessage {
    /// A fresh polled reading (or `None` when the source is unavailable).
    Loaded(Option<Battery>),
}

impl BatteryGroup {
    /// Apply a [`BatteryMessage`].
    pub fn update(&mut self, msg: BatteryMessage) -> Task<Message> {
        match msg {
            BatteryMessage::Loaded(data) => {
                self.battery = data;
                Task::none()
            }
        }
    }

    /// The battery leaf, or `None` if the source is unavailable.
    pub fn view<'a>(&self, config: &Config) -> Option<Element<'a, Message>> {
        leaf(config, self.battery.as_ref().map(|w| w as &dyn BarWidget))
    }
}
