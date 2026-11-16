mod battery_charge_gauge;
mod battery_time_reading;
mod cpu_tuning_sections;
mod estimate_columns;
mod power_draw_gauge;
mod power_profile_control;
mod profile_status_sections;
mod sqlite_charge_power_curve;
mod sqlite_power_draw_regression;

use battery_charge_gauge::{battery_hover_card, battery_reading_row};
use battery_time_reading::{BatteryTime, BatteryTimeReading, CurveLoaded};
use iced::{Subscription, Task};
use power_draw_gauge::PowerDraw;
use power_profile_control::{PowerProfileControl, PowerProfileInteraction, PowerProfileMessage};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    battery_power_profiles::{
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
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
        power_group::{
            cpu_tuning_sections::cpu_tuning_sections,
            power_draw_gauge::{PowerSource, charger_supply_sections},
        },
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        ongoing_toast::OngoingToastActionButton,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

const POWER_PROFILE_GLYPH: Phosphor = Phosphor::Lightning(Weight::Regular);
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
    time: Option<BatteryTime>,
    time_reading: BatteryTimeReading,
    power_profile: PowerProfileControl,
    charger: HoverFetchedSnapshot<ChargerSupply>,
    cpu_tuning: HoverFetchedSnapshot<CpuTuning>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum PowerMessage {
    Battery(SnapshotDelivery<Battery>),
    ChargerFetched(SnapshotDelivery<ChargerSupply>),
    CpuTuningFetched(SnapshotDelivery<CpuTuning>),
    ChargeCurveLoaded(CurveLoaded),
    PowerProfile(PowerProfileMessage),
}

#[derive(Debug, Clone)]
pub enum PowerInteraction {
    PowerProfile(PowerProfileInteraction),
    Expansion(WidgetGroupExpansionMessage),
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
        let task = match power_message {
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
            PowerMessage::PowerProfile(profile_message) => self
                .power_profile
                .update(profile_message, clock)
                .map(PowerMessage::PowerProfile),
        };
        self.estimate_time_left();
        task
    }

    pub fn interact(
        &mut self,
        interaction: PowerInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<PowerMessage> {
        let task = match interaction {
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
        };
        self.estimate_time_left();
        task
    }

    fn estimate_time_left(&mut self) {
        let battery = self.battery.get();
        self.time = battery.and_then(|battery| {
            self.time_reading
                .time_for(battery, self.power_profile.active_estimate())
        });
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
            self.time_reading
                .learn(battery, now)
                .map(PowerMessage::ChargeCurveLoaded),
            self.power_profile
                .observe_battery(battery, now)
                .map(PowerMessage::PowerProfile),
        ])
    }

    pub fn group_content<'a>(&'a self, config: &'a Config, clock: Clock) -> WidgetGroupContent<'a> {
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
        let group_summary = match self.power_profile.profile_status() {
            SubscribedSnapshot::SnapshotHeld { .. }
            | SubscribedSnapshot::SnapshotTroubled { .. } => GroupSummary::summarising(
                POWER_PROFILE_GLYPH.into(),
                self.power_profile_item(config),
            ),
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                GroupSummary::glyph(POWER_PROFILE_GLYPH.into(), placeholder.shown_state())
            }
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(POWER_PROFILE_GLYPH.into(), ReadingDisplayState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(POWER_PROFILE_GLYPH.into(), ReadingDisplayState::Off)
            }
        };
        let mut rows = Vec::new();
        match &self.battery {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let charge_row = battery_reading_row(held_snapshot);
                if charge_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.battery_charge_item(charge_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let charge_row = troubled_snapshot
                    .shown_over(battery_reading_row(&troubled_snapshot.held_snapshot), clock);
                if charge_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.battery_charge_item(charge_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.battery_charge_item(placeholder.reading_row(BATTERY_CHARGE_GLYPH.into())),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        if let Some(draw) = self.draw() {
            rows.push(self.power_draw_item(draw));
        }
        if let Some(time) = &self.time {
            rows.push(
                GroupItem::reading(time.reading_row(locale))
                    .ongoing_toast_action_buttons(switch_to_saving_profiles)
                    .with_hover_card(Box::new(move |_| {
                        self.battery.hover_card_with(time.hover_card())
                    }))
                    .refreshed_by(battery_refresh()),
            );
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Power,
            group_summary,
            &self.expansion,
            PowerInteraction::Expansion,
            rows,
        )
    }

    fn power_profile_item<'a>(&'a self, config: &'a Config) -> GroupItem<'a> {
        GroupItem::slider(
            self.power_profile.slider_control(),
            config,
            |control_message| {
                PowerInteraction::PowerProfile(PowerProfileInteraction::Control(control_message))
            },
        )
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

    fn battery_charge_item<'a>(&'a self, charge_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(charge_row)
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

    fn power_draw_item(&self, draw: PowerDraw) -> GroupItem<'_> {
        GroupItem::reading(draw.reading_row())
            .with_hover_card(Box::new(move |_| {
                self.draw().map_or_else(Vec::new, |current_draw| {
                    let mut sections = self.battery.hover_card_with(current_draw.hover_card());
                    if current_draw.power_source() != PowerSource::Battery {
                        sections
                            .extend(self.charger.sections_described_by(charger_supply_sections));
                    }
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
