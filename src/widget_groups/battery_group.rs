use iced::{Element, Subscription, Task};

use crate::{
    config::Config,
    data::{
        source::{
            Tooltip,
            sysfs_sensors::{self, battery::Battery, power::PowerDraw},
        },
        source_cell::SourceCell,
    },
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::reading_row::{Reading, ReadingRow},
};

#[derive(Default)]
pub struct BatteryGroup {
    charge: SourceCell<Battery>,
    draw: SourceCell<PowerDraw>,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Charge(Result<Option<Battery>, sysfs_sensors::Error>),
    Draw(Result<Option<PowerDraw>, sysfs_sensors::Error>),
    Tooltip(TooltipMessage),
}

impl BatteryGroup {
    pub fn subscription() -> Subscription<BatteryMessage> {
        Subscription::batch([
            SourceCell::subscription().map(BatteryMessage::Charge),
            SourceCell::subscription().map(BatteryMessage::Draw),
        ])
    }

    pub fn update(&mut self, msg: BatteryMessage) -> Task<BatteryMessage> {
        match msg {
            BatteryMessage::Charge(reading) => self.charge.receive(reading),
            BatteryMessage::Draw(reading) => self.draw.receive(reading),
            BatteryMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Battery => self.charge.tooltip(),
            HoverTarget::PowerDraw => self.draw.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, BatteryMessage>> {
        ReadingRow::new()
            .push(Reading::new(self.charge.display(), HoverTarget::Battery))
            .push(Reading::new(self.draw.display(), HoverTarget::PowerDraw))
            .slot(config, position, BatteryMessage::Tooltip)
    }
}
