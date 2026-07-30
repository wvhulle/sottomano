use iced::{Alignment, Element, Subscription, widget::Row};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        BarWidget, Tooltip,
        sysfs_sensors::{battery::Battery, power::PowerDraw},
    },
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widget_groups::source_cell::SourceCell,
    widgets::icon_gauge::source_reading,
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

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, BatteryMessage>> {
        let readings = [
            self.charge
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::Battery)),
            self.draw
                .get()
                .map(|w| (w as &dyn BarWidget, HoverTarget::PowerDraw)),
        ]
        .into_iter()
        .flatten()
        .filter_map(|(widget, target)| {
            let reading = source_reading(config, Some(widget), 1.0)?;
            Some(hoverable(reading, target, BatteryMessage::Tooltip))
        })
        .collect::<Vec<_>>();

        if readings.is_empty() {
            return None;
        }
        Some(
            Row::with_children(readings)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}
