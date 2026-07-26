use iced::{Element, Subscription};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, Tooltip, sysfs_sensors::battery::Battery},
    tooltip::{TooltipMessage, hoverable},
    widgets::icon_gauge::source_reading,
};

#[derive(Default)]
pub struct BatteryGroup {
    cell: SourceCell<Battery>,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Reading(Option<Battery>),
    Tooltip(TooltipMessage),
}

impl BatteryGroup {
    pub fn subscription() -> Subscription<BatteryMessage> {
        SourceCell::subscription().map(BatteryMessage::Reading)
    }

    pub fn update(&mut self, msg: BatteryMessage) {
        match msg {
            BatteryMessage::Reading(reading) => self.cell.load(reading),
            BatteryMessage::Tooltip(_) => {}
        }
    }

    pub fn tooltip_for(&self, id: &str) -> Option<Tooltip> {
        (id == "battery").then(|| self.cell.tooltip()).flatten()
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, BatteryMessage>> {
        let battery = self.cell.get()?;
        let reading = source_reading(config, Some(battery as &dyn BarWidget), 1.0)?;
        Some(hoverable(reading, "battery", BatteryMessage::Tooltip))
    }
}
