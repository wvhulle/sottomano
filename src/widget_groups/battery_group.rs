pub mod battery_charge_gauge;
pub mod power_draw_gauge;

use iced::{Element, Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        Source,
        sysfs_sensors::{self, battery::Battery, power::PowerDraw},
    },
    message_subscriptions::polling,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        last_poll::LastPoll,
        reading_row::{Reading, ReadingRow},
        tooltip_body::Tooltip,
    },
};

#[derive(Default)]
pub struct BatteryGroup {
    charge: LastPoll<Battery>,
    draw: LastPoll<PowerDraw>,
    installed: bool,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Installed(bool),
    Charge(Result<Battery, sysfs_sensors::Error>),
    Draw(Result<PowerDraw, sysfs_sensors::Error>),
    Tooltip(TooltipMessage),
}

impl BatteryGroup {
    pub fn probe() -> Task<BatteryMessage> {
        Task::future(async { BatteryMessage::Installed(Battery::available(&()).await) })
    }

    pub fn subscription(&self) -> Subscription<BatteryMessage> {
        if !self.installed {
            return Subscription::none();
        }
        Subscription::batch([
            polling::<Battery>(()).map(BatteryMessage::Charge),
            polling::<PowerDraw>(()).map(BatteryMessage::Draw),
        ])
    }

    pub fn update(&mut self, msg: BatteryMessage) -> Task<BatteryMessage> {
        match msg {
            BatteryMessage::Installed(installed) => self.installed = installed,
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
