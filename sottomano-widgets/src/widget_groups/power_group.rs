mod battery_charge_gauge;
mod battery_time_reading;
mod cpu_tuning_sections;
mod estimate_columns;
mod power_draw_gauge;
mod power_profile_control;
mod profile_status_sections;
mod sqlite_charge_power_curve;
mod sqlite_power_draw_regression;

use battery_time_reading::{BatteryTime, BatteryTimeReading, CurveLoaded};
use iced::{Subscription, Task};
use power_draw_gauge::PowerDraw;
use power_profile_control::{PowerProfileControl, PowerProfileMessage};
use sottomano_config::Config;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    battery_power_profiles::{
        cpu_tuning_platform_profile::CpuTuning,
        upower_battery_state::{Battery, ChargerSupply},
    },
    desktop_notifications::notification_data::ActionTint,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::LatestSnapshot,
        ongoing_toast::OngoingToastActionButton,
        phosphor::{Phosphor, Weight},
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
    battery: LatestSnapshot<Battery>,
    time: Option<BatteryTime>,
    time_reading: BatteryTimeReading,
    power_profile: PowerProfileControl,
    charger: HoverFetchedSnapshot<ChargerSupply>,
    cpu_tuning: HoverFetchedSnapshot<CpuTuning>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum PowerMessage {
    Battery(SnapshotDelivery<Battery>),
    ChargerFetched(SnapshotDelivery<ChargerSupply>),
    CpuTuningFetched(SnapshotDelivery<CpuTuning>),
    ChargeCurveLoaded(CurveLoaded),
    PowerProfile(PowerProfileMessage),
    Expansion(WidgetGroupExpansionMessage),
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
            snapshot_subscription::<Battery>(buses.clone()).map(PowerMessage::Battery),
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
        clock: Clock,
    ) -> Task<PowerMessage> {
        self.charger.stop();
        self.cpu_tuning.stop();
        match hovered {
            Some(HoverTarget::Power(PowerTarget::Draw)) => {
                self.charger
                    .start_hover_fetch((), PowerMessage::ChargerFetched, config, clock)
            }
            Some(HoverTarget::Power(PowerTarget::Profile)) => {
                buses.map_or_else(Task::none, |buses| {
                    self.cpu_tuning.start_hover_fetch(
                        buses.clone(),
                        PowerMessage::CpuTuningFetched,
                        config,
                        clock,
                    )
                })
            }
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, msg: PowerMessage, clock: Clock) -> Task<PowerMessage> {
        let task = match msg {
            PowerMessage::Battery(reading) => {
                self.battery.receive(reading, clock);
                self.observe_battery(clock)
            }
            PowerMessage::ChargerFetched(fetched) => {
                self.charger.receive(fetched, clock);
                Task::none()
            }
            PowerMessage::CpuTuningFetched(fetched) => {
                self.cpu_tuning.receive(fetched, clock);
                Task::none()
            }
            PowerMessage::ChargeCurveLoaded(curve) => {
                self.time_reading.loaded(curve);
                Task::none()
            }
            PowerMessage::PowerProfile(msg) => self
                .power_profile
                .update(msg, clock)
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

    fn observe_battery(&mut self, clock: Clock) -> Task<PowerMessage> {
        let now = clock.instant;
        let Some(battery) = self.battery.get() else {
            return Task::none();
        };
        Task::batch([
            self.time_reading
                .learn(battery, now)
                .map(PowerMessage::ChargeCurveLoaded),
            self.power_profile
                .observe_battery(battery, now)
                .map(PowerMessage::PowerProfile),
        ])
    }

    #[must_use]
    pub fn hover_card_for(&self, target: PowerTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            PowerTarget::Battery => self.battery.hover_card_with_groups(
                self.battery
                    .sections_described_by(|battery| vec![Section::from(&battery.identity)]),
                clock,
            ),
            PowerTarget::Draw => {
                let draw = self.draw()?;
                let charger = if draw.on_battery() {
                    None
                } else {
                    self.charger.sections(clock)
                };
                Some(
                    self.battery
                        .hover_card_with(draw.hover_card(clock))?
                        .with_groups(charger),
                )
            }
            PowerTarget::BatteryTime => self
                .battery
                .hover_card_with(self.time.as_ref()?.hover_card(clock)),
            PowerTarget::Profile => self
                .power_profile
                .hover_card(clock)
                .map(|hover_card| hover_card.with_groups(self.cpu_tuning.sections(clock))),
        }
    }

    pub fn group_content<'a>(&'a self, config: &'a Config) -> WidgetGroupContent<'a, PowerMessage> {
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
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Power,
            &self.expansion,
            PowerMessage::Expansion,
            [
                self.battery
                    .group_item(PowerTarget::Battery)
                    .map(GroupItem::previewed_on_handle),
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
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Lightning(Weight::Regular).into(),
        })
    }
}
