pub mod battery_charge_gauge;
pub mod battery_time_reading;
pub mod cpu_tuning_reading;
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
            cpu_tuning::CpuTuning,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hardware_section::HardwareEntries,
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        reading_alert::ReadingAlertActionButton,
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Default)]
pub struct PowerGroup {
    battery: LastPoll<Battery>,
    draw: Option<PowerDraw>,
    time: Option<BatteryTime>,
    time_reading: BatteryTimeReading,
    installed: bool,
    power_profile: PowerProfileControl,
    cpu_tuning: LastPoll<CpuTuning>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PowerMessage {
    Installed(bool),
    Battery(Result<Battery, sysfs_sensors::Error>),
    CpuTuning(Result<CpuTuning, sysfs_sensors::Error>),
    ChargeCurveLoaded(CurveLoaded),
    PowerProfile(PowerProfileMessage),
    Expansion(ExpansionMessage),
}

impl PowerGroup {
    pub fn collapse(&mut self) {
        if self.expansion.collapse() {
            self.power_profile.close();
        }
    }

    pub fn probe() -> Task<PowerMessage> {
        Task::batch([
            Task::future(async { PowerMessage::Installed(Battery::available(&()).await) }),
            PowerProfileControl::probe().map(PowerMessage::PowerProfile),
        ])
    }

    pub fn subscription(&self) -> Subscription<PowerMessage> {
        Subscription::batch([
            self.power_profile
                .subscription()
                .map(PowerMessage::PowerProfile),
            if self.installed {
                Subscription::run(battery::readings).map(PowerMessage::Battery)
            } else {
                Subscription::none()
            },
            if self.power_profile.slider().is_some() {
                polling::<CpuTuning>(()).map(PowerMessage::CpuTuning)
            } else {
                Subscription::none()
            },
        ])
    }

    pub fn update(&mut self, msg: PowerMessage) -> Task<PowerMessage> {
        let task = match msg {
            PowerMessage::Installed(installed) => {
                self.installed = installed;
                Task::none()
            }
            PowerMessage::Battery(reading) => {
                self.battery.receive(reading);
                self.observe_battery()
            }
            PowerMessage::CpuTuning(reading) => {
                self.cpu_tuning.receive(reading);
                Task::none()
            }
            PowerMessage::ChargeCurveLoaded(curve) => {
                self.time_reading.loaded(curve);
                Task::none()
            }
            PowerMessage::PowerProfile(msg) => self
                .power_profile
                .update(msg)
                .map(PowerMessage::PowerProfile),
            PowerMessage::Expansion(msg) => {
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

    fn observe_battery(&mut self) -> Task<PowerMessage> {
        let Some(battery) = self.battery.get() else {
            return Task::none();
        };
        Task::batch([
            self.time_reading
                .learn(battery)
                .map(PowerMessage::ChargeCurveLoaded),
            self.power_profile
                .observe_battery(battery)
                .map(PowerMessage::PowerProfile),
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
            HoverTarget::CpuTuning => self.cpu_tuning.tooltip(),
            _ => self.power_profile.tooltip_for(target),
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, PowerMessage> {
        let switch_to_saving_profiles =
            self.power_profile
                .profiles_saving_more_power()
                .map(|profile| ReadingAlertActionButton {
                    button_label: profile.title().to_owned(),
                    on_button_pressed: PowerMessage::PowerProfile(
                        PowerProfileMessage::SwitchToProfile(profile),
                    ),
                });
        GroupContent::new(
            &self.expansion,
            PowerMessage::Expansion,
            [
                self.battery.group_item(HoverTarget::Battery),
                self.draw
                    .as_ref()
                    .map(|draw| GroupItem::reading(draw, HoverTarget::PowerDraw)),
                self.time.as_ref().map(|time| {
                    GroupItem::reading(time, HoverTarget::BatteryTime)
                        .alert_action_buttons(switch_to_saving_profiles)
                }),
                self.power_profile.slider().map(|slider| {
                    GroupItem::slider(slider, config, |control| {
                        PowerMessage::PowerProfile(PowerProfileMessage::Control(control))
                    })
                }),
                self.power_profile
                    .slider()
                    .and_then(|_| self.cpu_tuning.group_item(HoverTarget::CpuTuning)),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon(Phosphor::Lightning(Weight::Regular).into()))
        .preview(self.battery.get().map(|battery| PreviewedValue::Hoverable {
            widget: battery,
            target: HoverTarget::Battery,
        }))
    }
}
