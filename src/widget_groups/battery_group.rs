pub mod battery_charge_gauge;
pub mod power_draw_gauge;
pub mod power_profile_control;

use iced::{Alignment, Element, Subscription, Task, widget::Row};
use power_profile_control::{PowerProfileControl, PowerProfileMessage};

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
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        tooltip_body::Tooltip,
    },
};

#[derive(Default)]
pub struct BatteryGroup {
    charge: LastPoll<Battery>,
    draw: LastPoll<PowerDraw>,
    installed: bool,
    power_profile: PowerProfileControl,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Installed(bool),
    Charge(Result<Battery, sysfs_sensors::Error>),
    Draw(Result<PowerDraw, sysfs_sensors::Error>),
    PowerProfile(PowerProfileMessage),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
}

impl BatteryGroup {
    pub fn is_animating(&self) -> bool {
        self.power_profile.is_animating()
    }

    pub fn probe() -> Task<BatteryMessage> {
        Task::batch([
            Task::future(async { BatteryMessage::Installed(Battery::available(&()).await) }),
            PowerProfileControl::probe().map(BatteryMessage::PowerProfile),
        ])
    }

    pub fn subscription(&self) -> Subscription<BatteryMessage> {
        let power_profile = self
            .power_profile
            .subscription()
            .map(BatteryMessage::PowerProfile);
        if !self.installed {
            return power_profile;
        }
        Subscription::batch([
            polling::<Battery>(()).map(BatteryMessage::Charge),
            polling::<PowerDraw>(()).map(BatteryMessage::Draw),
            power_profile,
        ])
    }

    pub fn update(&mut self, msg: BatteryMessage) -> Task<BatteryMessage> {
        match msg {
            BatteryMessage::Installed(installed) => self.installed = installed,
            BatteryMessage::Charge(reading) => self.charge.receive(reading),
            BatteryMessage::Draw(reading) => self.draw.receive(reading),
            BatteryMessage::PowerProfile(PowerProfileMessage::Tooltip(_))
            | BatteryMessage::Tooltip(_) => {}
            BatteryMessage::PowerProfile(msg) => {
                return self
                    .power_profile
                    .update(msg)
                    .map(BatteryMessage::PowerProfile);
            }
            BatteryMessage::Menu(msg) => {
                self.menu.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Battery => self.charge.tooltip(),
            HoverTarget::PowerDraw => self.draw.tooltip(),
            _ => self.power_profile.tooltip_for(target),
        }
    }

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, BatteryMessage>> {
        let charge_reading = move || {
            ReadingRow::new()
                .push(Reading::new(self.charge.display(), HoverTarget::Battery))
                .row(config, BatteryMessage::Tooltip)
        };
        charge_reading()?;

        let face = move || charge_reading().unwrap_or_else(Row::new).into();

        let content = move |reveal: f32| {
            let draw = ReadingRow::new()
                .opacity(reveal)
                .push(Reading::new(self.draw.display(), HoverTarget::PowerDraw))
                .row(config, BatteryMessage::Tooltip)
                .map(Element::from);
            let power_profile = self.power_profile.view(config, reveal).map(|element| {
                element.map(|m| match m {
                    PowerProfileMessage::Tooltip(t) => BatteryMessage::Tooltip(t),
                    m => BatteryMessage::PowerProfile(m),
                })
            });
            Row::with_children(draw.into_iter().chain(power_profile))
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };

        Some(
            self.menu
                .slot(face, content, BatteryMessage::Menu)
                .view(config, position),
        )
    }
}
