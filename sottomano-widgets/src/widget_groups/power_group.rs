mod battery_charge_gauge;
mod battery_time_reading;
mod cpu_tuning_sections;
mod estimate_columns;
mod power_draw_gauge;
mod power_profile_control;
mod profile_status_sections;

use battery_time_reading::{BatteryTime, BatteryTimeReading, CurveLoaded};
use iced::{Subscription, Task};
use power_draw_gauge::PowerDraw;
use power_profile_control::{PowerProfileControl, PowerProfileMessage};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    battery_power_profiles::{
        cpu_tuning_platform_profile::CpuTuning,
        upower_battery_state::{Battery, ChargerSupply},
    },
    desktop_notifications::notification_data::ActionTint,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        ongoing_toast::OngoingToastActionButton,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, Tooltip},
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
}

impl From<PowerTarget> for HoverTarget {
    fn from(target: PowerTarget) -> Self {
        HoverTarget::Power(target)
    }
}

#[derive(Default)]
pub struct PowerGroup {
    battery: ContinuousReading<Battery>,
    time: Option<BatteryTime>,
    time_reading: BatteryTimeReading,
    power_profile: PowerProfileControl,
    charger: OnHoverReading<ChargerSupply>,
    cpu_tuning: OnHoverReading<CpuTuning>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PowerMessage {
    Battery(SourceDelivery<Battery>),
    ChargerFetched(SourceDelivery<ChargerSupply>),
    CpuTuningFetched(SourceDelivery<CpuTuning>),
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

    pub fn subscription(buses: &DesktopBuses) -> Subscription<PowerMessage> {
        Subscription::batch([
            PowerProfileControl::subscription(buses).map(PowerMessage::PowerProfile),
            source_subscription::<Battery>(buses.clone()).map(PowerMessage::Battery),
        ])
    }

    pub fn refresh(target: PowerTarget, buses: &DesktopBuses) -> Task<PowerMessage> {
        match target {
            PowerTarget::Battery | PowerTarget::Draw | PowerTarget::BatteryTime => {
                first_delivery::<Battery>(buses.clone()).map(PowerMessage::Battery)
            }
            PowerTarget::Profile => {
                PowerProfileControl::refresh(buses).map(PowerMessage::PowerProfile)
            }
        }
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Task<PowerMessage> {
        self.charger.stop();
        self.cpu_tuning.stop();
        match hovered {
            Some(HoverTarget::Power(PowerTarget::Draw)) => {
                self.charger
                    .start_hover_fetch((), PowerMessage::ChargerFetched, config)
            }
            Some(HoverTarget::Power(PowerTarget::Profile)) => {
                buses.map_or_else(Task::none, |buses| {
                    self.cpu_tuning.start_hover_fetch(
                        buses.clone(),
                        PowerMessage::CpuTuningFetched,
                        config,
                    )
                })
            }
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, msg: PowerMessage) -> Task<PowerMessage> {
        let task = match msg {
            PowerMessage::Battery(reading) => {
                self.battery.receive_delivery(reading);
                self.observe_battery()
            }
            PowerMessage::ChargerFetched(fetched) => {
                self.charger.receive_delivery(fetched);
                Task::none()
            }
            PowerMessage::CpuTuningFetched(fetched) => {
                self.cpu_tuning.receive_delivery(fetched);
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
        task
    }

    fn draw(&self) -> Option<PowerDraw> {
        self.battery
            .get()
            .map(|battery| PowerDraw::of(battery, self.power_profile.expected_draw()))
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

    #[must_use]
    pub fn tooltip_for(&self, target: PowerTarget) -> Option<Tooltip> {
        match target {
            PowerTarget::Battery => self.battery.tooltip_with_sections(
                self.battery
                    .sections_described_by(|battery| vec![Section::from(&battery.identity)]),
            ),
            PowerTarget::Draw => {
                let draw = self.draw()?;
                let charger = if draw.on_battery() {
                    Vec::new()
                } else {
                    self.charger.sections()
                };
                Some(
                    self.battery
                        .tooltip_with(draw.tooltip())?
                        .with_sections(charger),
                )
            }
            PowerTarget::BatteryTime => self.battery.tooltip_with(self.time.as_ref()?.tooltip()),
            PowerTarget::Profile => self
                .power_profile
                .tooltip()
                .map(|tooltip| tooltip.with_sections(self.cpu_tuning.sections())),
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
                self.draw()
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
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Lightning(Weight::Regular).into(),
            handle: GroupHandle::Power,
        })
        .preview(self.battery.get().map(|battery| PreviewedValue::Hoverable {
            widget: battery,
            target: PowerTarget::Battery.into(),
        }))
    }
}
