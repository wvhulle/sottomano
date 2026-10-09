pub mod battery_charge_gauge;
pub mod battery_time_reading;
pub mod estimate_columns;
pub mod power_draw_gauge;
pub mod power_profile_control;

use battery_time_reading::{BatteryTime, BatteryTimeReading, CurveLoaded};
use iced::{Subscription, Task};
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
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hardware_section::HardwareRows,
        last_poll::LastPoll,
        reading_alert::ReadingAlertActionButton,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
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
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum BatteryMessage {
    Installed(bool),
    Battery(Result<Battery, sysfs_sensors::Error>),
    ChargeCurveLoaded(CurveLoaded),
    PowerProfile(PowerProfileMessage),
    Expansion(ExpansionMessage),
}

impl BatteryGroup {
    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
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
            BatteryMessage::PowerProfile(msg) => self
                .power_profile
                .update(msg)
                .map(BatteryMessage::PowerProfile),
            BatteryMessage::Expansion(msg) => {
                if self.expansion.update(msg) {
                    self.power_profile.close();
                }
                Task::none()
            }
        };
        let battery = self.battery.get();
        self.time = battery.and_then(|battery| {
            self.time_reading
                .time_for(battery, self.power_profile.active_estimate())
        });
        self.draw =
            battery.map(|battery| PowerDraw::of(battery, self.power_profile.expected_draw()));
        task
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
            HoverTarget::Battery => self.battery.tooltip_with_sections(&[self
                .battery
                .get()
                .map(|battery| &battery.identity)
                .hardware_section()]),
            HoverTarget::PowerDraw => self.battery.tooltip_with(self.draw.as_ref()?.tooltip()),
            HoverTarget::BatteryTime => self.battery.tooltip_with(self.time.as_ref()?.tooltip()),
            _ => self.power_profile.tooltip_for(target),
        }
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
    ) -> Option<WidgetGroup<'a, BatteryMessage>> {
        let time_item = self.time_widget().map(|time| GroupItem {
            alert_action_buttons: self
                .power_profile
                .profiles_saving_more_power()
                .map(|profile| ReadingAlertActionButton {
                    button_label: profile.title().to_owned(),
                    on_button_pressed: BatteryMessage::PowerProfile(
                        PowerProfileMessage::SwitchToProfile(profile),
                    ),
                })
                .collect(),
            ..GroupItem::reading(time, HoverTarget::BatteryTime)
        });
        let power_profile_item = self.power_profile.slider().map(|slider| {
            GroupItem::slider(slider, config, |m| {
                BatteryMessage::PowerProfile(PowerProfileMessage::Control(m))
            })
        });
        WidgetGroup::new(
            &self.expansion,
            BatteryMessage::Expansion,
            config,
            GroupHandle {
                face: HandleFace::Item(self.battery.reading_item(HoverTarget::Battery)?),
                preview: None,
                forwards_scroll: false,
            },
            &[
                self.draw
                    .as_ref()
                    .map(|draw| GroupItem::reading(draw as &dyn BarWidget, HoverTarget::PowerDraw)),
                time_item,
                power_profile_item,
            ],
        )
    }
}
