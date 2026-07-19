//! odin's `WidgetGroup/Battery.qml`: a single-widget static group. Owns its own
//! polled reading.

use iced::Element;

use crate::{
    bar_program::Message,
    data_sources::sysfs_sensors::Battery,
    user_config::Config,
    visual_elements::{as_bar_widget, leaf},
};

#[derive(Default)]
pub struct BatteryGroup {
    battery: Option<Battery>,
}

impl BatteryGroup {
    pub fn set(&mut self, data: Option<Battery>) {
        self.battery = data;
    }

    pub fn view<'a>(&self, config: &Config) -> Option<Element<'a, Message>> {
        leaf(config, self.battery.as_ref().map(as_bar_widget))
    }
}
