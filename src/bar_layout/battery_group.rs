use iced::Element;

use crate::{
    data_sources::{BarWidget, sysfs_sensors::battery::Battery},
    tooltip::{TooltipMessage, hoverable},
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
    Tooltip(TooltipMessage),
}

impl BatteryGroup {
    pub fn update(&mut self, msg: BatteryMessage) {
        match msg {
            BatteryMessage::Loaded(data) => self.battery = data,
            BatteryMessage::Tooltip(_) => {}
        }
    }

    pub fn view<'a>(&self, config: &Config) -> Option<Element<'a, BatteryMessage>> {
        let battery = self.battery.as_ref()?;
        let reading = source_reading(config, Some(battery as &dyn BarWidget), 1.0)?;
        Some(hoverable(reading, battery.tooltip(), BatteryMessage::Tooltip))
    }
}
