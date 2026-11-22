mod battery_charge_gauge;
mod battery_time_indicator;
mod charger_connection;
mod cpu_tuning_sections;
mod estimate_columns;
mod power_draw_gauge;
mod power_profile_control;
mod profile_status_sections;
mod sample_spacing;
mod sqlite_charge_power_curve;

use battery_charge_gauge::{battery_hover_card, battery_indicator};
use battery_time_indicator::{BatteryTime, BatteryTimeIndicator};
use chrono::Locale;
use iced::{Subscription, Task};
use power_draw_gauge::PowerDraw;
use power_profile_control::{PowerProfileControl, PowerProfileInteraction, PowerProfileMessage};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    battery_power_profiles::{
        battery_time_forecast::ChargeCurve,
        cpu_tuning_platform_profile::CpuTuning,
        upower_battery_state::{Battery, ChargerSupply},
    },
    desktop_notifications::notification_data::ActionTint,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            ExpansionChange, IndicatorGroupContent, IndicatorGroupExpansion,
            IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        power_group::{
            cpu_tuning_sections::cpu_tuning_sections, power_draw_gauge::charger_supply_sections,
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, ongoing_toast::OngoingToastActionButton,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

const BATTERY_CHARGE_GLYPH: Phosphor = Phosphor::BatteryFull(Weight::Regular);

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

    fn battery_time(&self) -> Option<BatteryTime> {
        self.time_indicator
            .time_for(self.battery.get()?, self.power_profile.active_estimate())
    }

    fn draw(&self) -> Option<PowerDraw> {
        self.battery
            .get()
            .map(|battery| PowerDraw::battery_draw(battery, self.power_profile.expected_draw()))
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
        let switch_to_saving_profiles =
            self.power_profile
                .profiles_saving_more_power()
                .map(|profile| OngoingToastActionButton {
                    button_label: profile.title().to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: PowerInteraction::PowerProfile(
                        PowerProfileInteraction::SwitchToProfile(profile),
                    ),
                });
        let headline_indicator = match &self.battery {
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
        };
        let mut lead_indicators = Vec::new();
        if self.power_profile.status.get().is_some() {
            lead_indicators.push(self.power_profile_indicator(config));
        }
        let mut rows = Vec::new();
        if !self.battery.awaits_first_delivery() {
            let charge_indicator = self.battery.indicator(
                |held_snapshot| battery_indicator(held_snapshot),
                BATTERY_CHARGE_GLYPH.into(),
            );
            if charge_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.battery_charge_indicator(charge_indicator));
            }
        }
        if let Some(draw) = self.draw() {
            rows.push(self.power_draw_indicator(draw, locale));
        }
        if let Some(time) = self.battery_time() {
            rows.push(
                time.indicator(locale)
                    .ongoing_toast_action_buttons(switch_to_saving_profiles)
                    .with_hover_card(Box::new(move |_| {
                        self.battery.hover_card_with(time.hover_card())
                    }))
                    .refreshed_by(battery_refresh()),
            );
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Power,
            headline_indicator,
            &self.expansion,
            PowerInteraction::Expansion,
            lead_indicators.into_iter().chain(rows),
        )
    }

    fn power_profile_indicator<'view>(&'view self, config: &'view Config) -> GroupIndicator<'view> {
        GroupIndicator::slider(&self.power_profile.control, config, |control_message| {
            PowerInteraction::PowerProfile(PowerProfileInteraction::Control(control_message))
        })
        .with_hover_card(Box::new(move |_| {
            let mut sections = self.power_profile.hover_card();
            sections.extend(self.cpu_tuning.sections_described_by(cpu_tuning_sections));
            sections
        }))
        .fetching_on_hover(
            PowerInteraction::OnBuses(PowerBusRequest::StartCpuTuningFetch),
            PowerInteraction::StopHoverFetches,
        )
        .refreshed_by(PowerInteraction::OnBuses(PowerBusRequest::Refresh(
            PowerRefreshedSource::Profile,
        )))
    }

    fn battery_charge_indicator<'view>(
        &'view self,
        charge_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        charge_indicator
            .with_hover_card(Box::new(move |_| {
                let mut sections = self.battery.sections_described_by(battery_hover_card);
                sections.extend(
                    self.battery
                        .sections_described_by(|battery| vec![Section::from(battery)]),
                );
                sections
            }))
            .refreshed_by(battery_refresh())
    }

    fn power_draw_indicator(&self, draw: PowerDraw, locale: Locale) -> GroupIndicator<'_> {
        draw.indicator(locale)
            .with_hover_card(Box::new(move |_| {
                self.draw().map_or_else(Vec::new, |current_draw| {
                    let mut sections = if current_draw.connection().feeds_from_charger() {
                        self.charger.sections_described_by(charger_supply_sections)
                    } else {
                        Vec::new()
                    };
                    sections.extend(self.battery.hover_card_with(current_draw.hover_card()));
                    sections
                })
            }))
            .fetching_on_hover(
                PowerInteraction::StartChargerFetch,
                PowerInteraction::StopHoverFetches,
            )
            .refreshed_by(battery_refresh())
    }
}

fn battery_refresh() -> PowerInteraction {
    PowerInteraction::OnBuses(PowerBusRequest::Refresh(PowerRefreshedSource::Battery))
}
