mod battery_charge_indicator;
mod battery_time_indicator;
mod charger_connection;
mod estimate_columns;
mod power_draw_indicator;
mod power_profile_indicator;
mod sample_spacing;

use battery_charge_indicator::{BATTERY_CHARGE_GLYPH, battery_indicator};
use battery_time_indicator::BatteryTimeIndicator;
use iced::{Subscription, Task};
use power_profile_indicator::power_profile_control::{
    PowerProfileControl, PowerProfileInteraction, PowerProfileMessage,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    battery_power_profiles::{
        battery_time_forecast::ChargeCurve,
        cpu_tuning_platform_profile::CpuTuning,
        upower_battery_state::{Battery, ChargerSupply},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            ExpansionChange, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerRefreshedSource {
    Battery,
    Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerBusRequest {
    StartCpuTuningFetch,
    Refresh(PowerRefreshedSource),
}

#[derive(Default)]
pub struct PowerGroup {
    battery: SubscribedSnapshot<Battery>,
    time_indicator: BatteryTimeIndicator,
    power_profile: PowerProfileControl,
    charger: HoverFetchedSnapshot<ChargerSupply>,
    cpu_tuning: HoverFetchedSnapshot<CpuTuning>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum PowerMessage {
    Battery(SnapshotDelivery<Battery>),
    ChargerFetched(SnapshotDelivery<ChargerSupply>),
    CpuTuningFetched(SnapshotDelivery<CpuTuning>),
    ChargeCurveLoaded(Box<ChargeCurve>),
    PowerProfile(PowerProfileMessage),
}

#[derive(Debug, Clone)]
pub enum PowerInteraction {
    PowerProfile(PowerProfileInteraction),
    Expansion(IndicatorGroupExpansionMessage),
    StartChargerFetch,
    StopHoverFetches,
    OnBuses(PowerBusRequest),
}

impl PowerGroup {
    pub fn collapse(&mut self) {
        if self.expansion.collapse() == ExpansionChange::JustCollapsed {
            self.power_profile.close();
        }
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<PowerMessage> {
        Subscription::batch([
            PowerProfileControl::subscription(buses).map(PowerMessage::PowerProfile),
            snapshot_subscription::<Battery>(buses.clone()).map(PowerMessage::Battery),
        ])
    }

    pub fn on_buses(
        &mut self,
        request: PowerBusRequest,
        config: &Config,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<PowerMessage> {
        match request {
            PowerBusRequest::StartCpuTuningFetch => self.cpu_tuning.start_hover_fetch(
                buses.clone(),
                PowerMessage::CpuTuningFetched,
                config,
                clock,
            ),
            PowerBusRequest::Refresh(PowerRefreshedSource::Battery) => {
                first_delivery::<Battery>(buses.clone()).map(PowerMessage::Battery)
            }
            PowerBusRequest::Refresh(PowerRefreshedSource::Profile) => {
                PowerProfileControl::refresh(buses).map(PowerMessage::PowerProfile)
            }
        }
    }

    pub fn update(&mut self, power_message: PowerMessage, clock: Clock) -> Task<PowerMessage> {
        match power_message {
            PowerMessage::Battery(sample) => {
                self.battery.receive(sample, clock);
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
                self.time_indicator.loaded(curve);
                Task::none()
            }
            PowerMessage::PowerProfile(profile_message) => self
                .power_profile
                .update(profile_message, clock)
                .map(PowerMessage::PowerProfile),
        }
    }

    pub fn interact(
        &mut self,
        interaction: PowerInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<PowerMessage> {
        match interaction {
            PowerInteraction::PowerProfile(profile_interaction) => self
                .power_profile
                .interact(profile_interaction)
                .map(PowerMessage::PowerProfile),
            PowerInteraction::Expansion(expansion_message) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.power_profile.close();
                }
                Task::none()
            }
            PowerInteraction::StartChargerFetch => {
                self.charger
                    .start_hover_fetch((), PowerMessage::ChargerFetched, config, clock)
            }
            PowerInteraction::StopHoverFetches => {
                self.charger.stop();
                self.cpu_tuning.stop();
                Task::none()
            }
            PowerInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn observe_battery(&mut self, clock: Clock) -> Task<PowerMessage> {
        let now = clock.instant;
        let Some(battery) = self.battery.get() else {
            return Task::none();
        };
        Task::batch([
            self.time_indicator
                .learn(battery, now)
                .map(PowerMessage::ChargeCurveLoaded),
            self.power_profile
                .observe_battery(battery, now)
                .map(PowerMessage::PowerProfile),
        ])
    }

    pub fn group_content<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorGroupContent<'view> {
        let locale = config.calendar.locale;
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Power,
            self.headline_indicator(config),
            &self.expansion,
            PowerInteraction::Expansion,
            [
                self.power_profile_listing(config),
                self.battery_charge_listing(),
                self.power_draw_listing(locale),
                self.battery_time_listing(locale),
            ]
            .into_iter()
            .collect(),
        )
    }

    fn headline_indicator<'view>(&'view self, config: &'view Config) -> GroupIndicator<'view> {
        match &self.battery {
            SubscribedSnapshot::Available { .. } => self
                .battery
                .indicator(battery_indicator, BATTERY_CHARGE_GLYPH.into()),
            SubscribedSnapshot::Unavailable { .. } => match &self.power_profile.status {
                SubscribedSnapshot::Available { .. } => self
                    .power_profile
                    .control
                    .indicator(config.calendar.locale)
                    .with_headline_glyph(Phosphor::Plug(Weight::Regular).into()),
                SubscribedSnapshot::Unavailable { cause } => {
                    cause.indicator(Phosphor::Plug(Weight::Regular).into())
                }
            },
        }
    }
}

fn battery_refresh() -> PowerInteraction {
    PowerInteraction::OnBuses(PowerBusRequest::Refresh(PowerRefreshedSource::Battery))
}
