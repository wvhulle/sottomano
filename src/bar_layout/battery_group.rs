use iced::{Element, Subscription, Task};

use crate::{
    bar_layout::source_cell::{SourceCell, SourceMessage},
    data_sources::{BarWidget, sysfs_sensors::battery::Battery},
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    user_config::Config,
    widgets::icon_gauge::source_reading,
};

#[derive(Default)]
pub struct BatteryGroup {
    cell: SourceCell<Battery>,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Source(SourceMessage<Battery>),
    Tooltip(TooltipMessage),
}

impl BatteryGroup {
    pub fn init() -> Task<BatteryMessage> {
        SourceCell::init().map(BatteryMessage::Source)
    }

    pub fn subscription() -> Subscription<BatteryMessage> {
        SourceCell::subscription().map(BatteryMessage::Source)
    }

    pub fn update(&mut self, msg: BatteryMessage) -> Task<BatteryMessage> {
        match msg {
            BatteryMessage::Source(msg) => self.cell.update(msg).map(BatteryMessage::Source),
            BatteryMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn reading(&self) -> Option<&Battery> {
        self.cell.get()
    }

    pub fn view<'a>(&self, config: &Config) -> Option<Element<'a, BatteryMessage>> {
        let battery = self.cell.get()?;
        let reading = source_reading(config, Some(battery as &dyn BarWidget), 1.0)?;
        Some(hoverable(
            reading,
            TooltipSource::Battery,
            BatteryMessage::Tooltip,
        ))
    }
}
