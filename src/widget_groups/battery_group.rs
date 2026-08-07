use iced::{Element, Subscription};

use crate::{
    config::Config,
    data::source::{
        Tooltip,
        sysfs_sensors::{battery::Battery, power::PowerDraw},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage},
    },
    widget_groups::{pills::Pills, source_cell::SourceCell},
};

#[derive(Default)]
pub struct BatteryGroup {
    charge: SourceCell<Battery>,
    draw: SourceCell<PowerDraw>,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Charge(Option<Battery>),
    Draw(Option<PowerDraw>),
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for BatteryMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
}

impl BatteryGroup {
    pub fn subscription() -> Subscription<BatteryMessage> {
        Subscription::batch([
            SourceCell::subscription().map(BatteryMessage::Charge),
            SourceCell::subscription().map(BatteryMessage::Draw),
        ])
    }

    pub fn update(&mut self, msg: BatteryMessage) {
        match msg {
            BatteryMessage::Charge(reading) => self.charge.load(reading),
            BatteryMessage::Draw(reading) => self.draw.load(reading),
            BatteryMessage::Tooltip(_) => {}
        }
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
        Pills::new()
            .push(self.charge.get(), HoverTarget::Battery)
            .push(self.draw.get(), HoverTarget::PowerDraw)
            .slot(config, position)
    }
}
