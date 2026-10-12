mod battery_charge_gauge;
mod battery_time_reading;
mod cpu_tuning_reading;
mod estimate_columns;
mod power_draw_gauge;
mod power_profile_control;

use battery_time_reading::{BatteryTime, BatteryTimeReading, CurveLoaded};
use cpu_tuning_reading::CpuTuningEntry;
use iced::{Subscription, Task};
use power_draw_gauge::PowerDraw;
use power_profile_control::{PowerProfileControl, PowerProfileMessage};

use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        source::{
            Source,
            sysfs_sensors::{
                self,
                battery::{self, Battery, ChargerSupply},
                cpu_tuning::CpuTuning,
            },
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, polled},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        hardware_section::HardwareEntries,
        on_hover_reading::OnHoverReading,
        ongoing_toast::OngoingToastActionButton,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Tooltip, TooltipBody},
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerTarget {
    Battery,
    Draw,
    BatteryTime,
    Profile,
    CpuTuning,
}

impl From<PowerTarget> for HoverTarget {
    fn from(target: PowerTarget) -> Self {
        HoverTarget::Power(target)
    }
}

#[derive(Default)]
pub struct PowerGroup {
    battery: ContinuousReading<Battery>,
    draw: Option<PowerDraw>,
    time: Option<BatteryTime>,
    time_reading: BatteryTimeReading,
    installed: bool,
    power_profile: PowerProfileControl,
    charger: OnHoverReading<ChargerSupply>,
    cpu_tuning: OnHoverReading<CpuTuning>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PowerMessage {
    Installed(bool),
    Battery(Result<Battery, sysfs_sensors::Error>),
    ChargerFetched(Result<ChargerSupply, String>),
    CpuTuningFetched(Result<CpuTuning, String>),
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

    pub fn subscription(&self, forced: Option<ForcedRefresh>) -> Subscription<PowerMessage> {
        Subscription::batch([
            self.power_profile
                .subscription(forced)
                .map(PowerMessage::PowerProfile),
            if self.installed {
                Subscription::run(battery::readings).map(PowerMessage::Battery)
            } else {
                Subscription::none()
            },
        ])
    }

    pub fn hover(&mut self, hovered: Option<&HoverTarget>, config: &Config) -> Task<PowerMessage> {
        self.charger.stop();
        self.cpu_tuning.stop();
        match hovered {
            Some(HoverTarget::Power(PowerTarget::Draw)) => self.charger.fetch(
                polled::<ChargerSupply>(()),
                PowerMessage::ChargerFetched,
                config,
            ),
            Some(HoverTarget::Power(PowerTarget::CpuTuning)) => self.cpu_tuning.fetch(
                polled::<CpuTuning>(()),
                PowerMessage::CpuTuningFetched,
                config,
            ),
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, msg: PowerMessage) -> Task<PowerMessage> {
        let task = match msg {
            PowerMessage::Installed(installed) => {
                self.installed = installed;
                Task::none()
            }
            PowerMessage::Battery(reading) => {
                self.battery.push(reading);
                self.observe_battery()
            }
            PowerMessage::ChargerFetched(fetched) => {
                self.charger.receive(fetched);
                Task::none()
            }
            PowerMessage::CpuTuningFetched(fetched) => {
                self.cpu_tuning.receive(fetched);
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

    pub fn tooltip_for(&self, target: PowerTarget) -> Option<Tooltip> {
        match target {
            PowerTarget::Battery => self.battery.tooltip_with_sections([self
                .battery
                .get()
                .map(|battery| &battery.identity)
                .hardware_section()]),
            PowerTarget::Draw => {
                let draw = self.draw.as_ref()?;
                let charger = if draw.on_battery() {
                    Vec::new()
                } else {
                    self.charger.sections(ChargerSupply::sections)
                };
                self.battery
                    .tooltip_with(draw.tooltip().with_sections(charger))
            }
            PowerTarget::BatteryTime => self.battery.tooltip_with(self.time.as_ref()?.tooltip()),
            PowerTarget::CpuTuning => Some(Tooltip::of(TooltipBody::sections(
                self.cpu_tuning.sections(CpuTuning::sections),
            ))),
            PowerTarget::Profile => self.power_profile.tooltip(),
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> GroupContent<'a, PowerMessage> {
        let switch_to_saving_profiles =
            self.power_profile
                .profiles_saving_more_power()
                .map(|profile| OngoingToastActionButton {
                    button_label: profile.title().to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: PowerMessage::PowerProfile(
                        PowerProfileMessage::SwitchToProfile(profile),
                    ),
                });
        GroupContent::new(
            &self.expansion,
            PowerMessage::Expansion,
            [
                self.battery.group_item(PowerTarget::Battery),
                self.draw
                    .as_ref()
                    .map(|draw| GroupItem::reading(draw, PowerTarget::Draw)),
                self.time.as_ref().map(|time| {
                    GroupItem::reading(time, PowerTarget::BatteryTime)
                        .ongoing_toast_action_buttons(switch_to_saving_profiles)
                }),
                self.power_profile.slider().map(|slider| {
                    GroupItem::slider(slider, config, |control| {
                        PowerMessage::PowerProfile(PowerProfileMessage::Control(control))
                    })
                }),
                self.power_profile
                    .slider()
                    .map(|_| GroupItem::reading(CpuTuningEntry, PowerTarget::CpuTuning)),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon(
            Phosphor::Lightning(Weight::Regular).into(),
        ))
        .preview(self.battery.get().map(|battery| PreviewedValue::Hoverable {
            widget: battery,
            target: PowerTarget::Battery.into(),
        }))
    }
}
