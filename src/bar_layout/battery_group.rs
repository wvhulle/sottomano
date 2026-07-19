use iced::{Element, Task};

use crate::{
    bar_program::Message,
    data_sources::{BarWidget, sysfs_sensors::Battery},
    user_config::Config,
    widgets::source_reading,
};

#[derive(Default)]
pub struct BatteryGroup {
    battery: Option<Battery>,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Loaded(Option<Battery>),
}

impl BatteryGroup {
    pub fn update(&mut self, msg: BatteryMessage) -> Task<Message> {
        match msg {
            BatteryMessage::Loaded(data) => {
                self.battery = data;
                Task::none()
            }
        }
    }

    pub fn view<'a>(&self, config: &Config) -> Option<Element<'a, Message>> {
        source_reading(config, self.battery.as_ref().map(|w| w as &dyn BarWidget))
    }
}
