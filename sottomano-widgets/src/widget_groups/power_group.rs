use crate::{
    group_items::reading_display_state::ReadingDisplayState, widgets::icon_glyph::IconGlyph,
};
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
    group_items::group_item::GroupItem,
    hover_cards::hover_card_body::Section,
    widget_groups::{
        power_group::{
            cpu_tuning_sections::cpu_tuning_sections,
            power_draw_gauge::{PowerSource, charger_supply_sections},
        },
        widget_group_content::{
            ExpansionChange, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
            WidgetGroupFace,
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
    Interaction(PowerInteraction),
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

    pub fn update(
        &mut self,
        power_message: PowerMessage,
        config: &Config,
        clock: Clock,
    ) -> Task<PowerMessage> {
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
            PowerMessage::Interaction(PowerInteraction::PowerProfile(profile_interaction)) => self
                .power_profile
                .update(PowerProfileMessage::Interaction(profile_interaction), clock)
                .map(PowerMessage::PowerProfile),
            PowerMessage::Interaction(PowerInteraction::Expansion(expansion_message)) => {
                if self.expansion.update(expansion_message) == ExpansionChange::JustCollapsed {
                    self.power_profile.close();
                }
                Task::none()
            }
            PowerMessage::Interaction(PowerInteraction::StartChargerFetch) => self
                .charger
                .start_hover_fetch((), PowerMessage::ChargerFetched, config, clock),
            PowerMessage::Interaction(PowerInteraction::StopHoverFetches) => {
                self.charger.stop();
                self.cpu_tuning.stop();
                Task::none()
            }
            PowerMessage::Interaction(PowerInteraction::OnBuses(_)) => Task::none(),
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

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a, PowerInteraction> {
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
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Power,
            &self.expansion,
            PowerInteraction::Expansion,
            [
                self.battery
                    .group_item(IconGlyph::default(), battery_reading_row, clock)
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            let mut sections =
                                self.battery.sections_described_by(battery_hover_card);
                            sections.extend(
                                self.battery
                                    .sections_described_by(|battery| vec![Section::from(battery)]),
                            );
                            sections
                        }))
                        .refreshed_by(battery_refresh())
                        .previewed_on_handle()
                    }),
                self.draw().map(|draw| {
                    GroupItem::reading(draw.reading_row())
                        .with_hover_card(Box::new(move |_| {
                            self.draw().map_or_else(Vec::new, |current_draw| {
                                let mut sections =
                                    self.battery.hover_card_with(current_draw.hover_card());
                                if current_draw.power_source() != PowerSource::Battery {
                                    sections.extend(
                                        self.charger.sections_described_by(charger_supply_sections),
                                    );
                                }
                                sections
                            })
                        }))
                        .fetching_on_hover(
                            PowerInteraction::StartChargerFetch,
                            PowerInteraction::StopHoverFetches,
                        )
                        .refreshed_by(battery_refresh())
                }),
                self.time.as_ref().map(|time| {
                    GroupItem::reading(time.reading_row(locale))
                        .ongoing_toast_action_buttons(switch_to_saving_profiles)
                        .with_hover_card(Box::new(move |_| {
                            self.battery.hover_card_with(time.hover_card())
                        }))
                        .refreshed_by(battery_refresh())
                }),
                self.power_profile.slider().map(|slider| {
                    GroupItem::slider(slider, config, |control_message| {
                        PowerInteraction::PowerProfile(PowerProfileInteraction::Control(
                            control_message,
                        ))
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
                }),
            ]
            .into_iter()
            .flatten(),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::Lightning(Weight::Regular).into(),
            state: ReadingDisplayState::Idle,
        })
    }
}

fn battery_refresh() -> PowerInteraction {
    PowerInteraction::OnBuses(PowerBusRequest::Refresh(PowerRefreshedSource::Battery))
}
