pub mod battery_charge_gauge;
pub mod battery_time_reading;
pub mod estimate_columns;
pub mod power_draw_gauge;
pub mod power_profile_control;

use battery_time_reading::{BatteryTime, BatteryTimeReading, CurveLoaded};
use iced::{Alignment, Element, Subscription, Task, widget::Row};
use power_draw_gauge::PowerDraw;
use power_profile_control::{PowerProfileControl, PowerProfileMessage};

use crate::{
    config::Config,
    data::source::{
        Source,
        sysfs_sensors::{
            self,
            battery::{self, Battery},
        },
    },
    wayland_layers::{
        notifications::NotificationMessage,
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        bar_widget::BarWidget,
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        severity_change::{severity_change, severity_of},
        tooltip_body::Tooltip,
    },
};

#[derive(Default)]
pub struct BatteryGroup {
    battery: LastPoll<Battery>,
    draw: Option<PowerDraw>,
    time: Option<BatteryTime>,
    time_reading: BatteryTimeReading,
    installed: bool,
    power_profile: PowerProfileControl,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Installed(bool),
    Battery(Result<Battery, sysfs_sensors::Error>),
    ChargeCurveLoaded(CurveLoaded),
    PowerProfile(PowerProfileMessage),
    Menu(MenuMessage),
    Tooltip(TooltipMessage),
    Notification(NotificationMessage),
}

impl BatteryGroup {
    pub fn close_menu(&mut self) {
        if self.menu.close() {
            self.power_profile.close();
        }
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
            Subscription::run(battery::readings).map(BatteryMessage::Battery),
            power_profile,
        ])
    }

    pub fn update(&mut self, msg: BatteryMessage) -> Task<BatteryMessage> {
        let task = match msg {
            BatteryMessage::Installed(installed) => {
                self.installed = installed;
                Task::none()
            }
            BatteryMessage::Battery(reading) => {
                self.battery.receive(reading);
                self.observe_battery()
            }
            BatteryMessage::ChargeCurveLoaded(curve) => {
                self.time_reading.loaded(curve);
                Task::none()
            }
            BatteryMessage::PowerProfile(PowerProfileMessage::Tooltip(_))
            | BatteryMessage::Tooltip(_)
            | BatteryMessage::Notification(_) => Task::none(),
            BatteryMessage::PowerProfile(msg) => self
                .power_profile
                .update(msg)
                .map(BatteryMessage::PowerProfile),
            BatteryMessage::Menu(msg) => {
                if self.menu.update(msg) {
                    self.power_profile.close();
                }
                Task::none()
            }
        };
        let time_severity_before = severity_of(self.time_widget());
        let battery = self.battery.get();
        self.time = battery.and_then(|battery| {
            self.time_reading
                .time_for(battery, self.power_profile.active_estimate())
        });
        self.draw =
            battery.map(|battery| PowerDraw::of(battery, self.power_profile.expected_draw()));
        let alert = severity_change(
            &HoverTarget::BatteryTime,
            "Battery running out",
            time_severity_before,
            self.time_widget(),
        )
        .map_or_else(Task::none, |message| {
            Task::done(BatteryMessage::Notification(message))
        });
        Task::batch([task, alert])
    }

    fn time_widget(&self) -> Option<&dyn BarWidget> {
        self.time.as_ref().map(|time| time as &dyn BarWidget)
    }

    fn observe_battery(&mut self) -> Task<BatteryMessage> {
        let Some(battery) = self.battery.get() else {
            return Task::none();
        };
        Task::batch([
            self.time_reading
                .learn(battery)
                .map(BatteryMessage::ChargeCurveLoaded),
            self.power_profile
                .observe_battery(battery)
                .map(BatteryMessage::PowerProfile),
        ])
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Battery => self.battery.tooltip(),
            HoverTarget::PowerDraw => self.battery.tooltip_with(self.draw.as_ref()?.tooltip()),
            HoverTarget::BatteryTime => self.battery.tooltip_with(self.time.as_ref()?.tooltip()),
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
                .push(Reading::new(self.battery.display(), HoverTarget::Battery))
                .row(config, BatteryMessage::Tooltip)
        };
        charge_reading()?;

        let face = move || charge_reading().unwrap_or_else(Row::new).into();

        let content = move |reveal: f32| {
            let draw = ReadingRow::new()
                .opacity(reveal)
                .push(Reading::new(
                    self.draw.as_ref().map(|draw| draw as &dyn BarWidget),
                    HoverTarget::PowerDraw,
                ))
                .push(Reading::new(self.time_widget(), HoverTarget::BatteryTime))
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
                .alert(severity_of(self.time_widget()))
                .view(config, position),
        )
    }
}
