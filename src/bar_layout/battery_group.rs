use iced::{Element, Task, window::Id};

use crate::{
    bar_program::Message,
    data_sources::{BarWidget, sysfs_sensors::battery::Battery},
    tooltip::hoverable,
    user_config::Config,
    widgets::icon_gauge::source_reading,
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

    pub fn view<'a>(&self, config: &Config, surface: Id) -> Option<Element<'a, Message>> {
        let battery = self.battery.as_ref()?;
        let reading = source_reading(config, Some(battery as &dyn BarWidget), 1.0)?;
        Some(hoverable(reading, surface, battery.tooltip()))
    }
}
