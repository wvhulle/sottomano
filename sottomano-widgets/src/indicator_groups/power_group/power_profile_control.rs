use std::{collections::HashMap, iter::once, ops::RangeInclusive, time::Instant};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::battery_power_profiles::power_profile_selection;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    battery_power_profiles::{
        battery_time_forecast::{Estimate, RegressionState, time_until_empty},
        power_profiles_daemon::{Conditions, Profile, ProfileStatus},
        upower_battery_state::{Battery, ChargeStatus},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use uom::si::{f64::Power, power::watt};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::slider_control::{Action, ControlMessage, SliderControl},
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    indicator_groups::power_group::{
        estimate_columns::{ESTIMATE_HEADINGS, Forecast, Unforecastable},
        power_draw_gauge::LaptopDrawEstimate,
        profile_status_sections::profile_status_sections,
        sample_spacing::SampleSpacing,
        sqlite_power_draw_regression,
    },
    widgets::{
        keyed_record_table::TableRecord, subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const RANGE: RangeInclusive<f32> = 0.0..=100.0;
const STEP: f32 = 50.0;

fn profile_face(profile: Profile) -> Phosphor {
    match profile {
        Profile::PowerSaver => Phosphor::Leaf(Weight::Regular),
        Profile::Balanced => Phosphor::Scales(Weight::Regular),
        Profile::Performance => Phosphor::Rocket(Weight::Regular),
    }
}
const MIN_SAMPLE_W: f64 = 0.5;

pub struct PowerProfileControl {
    pub control: SliderControl,
    pub status: SubscribedSnapshot<ProfileStatus>,
    conditions: ReportedConditions,
    battery: ObservedBattery,
    profile_models: DrainModelLearning,
    estimates: HashMap<Profile, Option<Estimate>>,
}

#[derive(Debug, Clone, Copy)]
enum ReportedConditions {
    Unreported,
    Reported(Conditions),
}

enum ObservedBattery {
    NotObserved,
    Observed(Battery),
}

enum DrainModelLearning {
    NotLoaded,
    Loading,
    Learning {
        models: HashMap<Profile, RegressionState>,
        spacing: SampleSpacing,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileActivity {
    Active,
    Inactive,
}

impl Default for PowerProfileControl {
    fn default() -> Self {
        Self {
            control: SliderControl::new("Power profile", RANGE, profile_face(Profile::Balanced))
                .slider_step(STEP)
                .scroll_step(STEP)
                .icon_only()
                .readout(|value, _| Profile::nearest_position_profile(value).title().to_string())
                .glyph(|value, _| profile_face(Profile::nearest_position_profile(value))),
            status: SubscribedSnapshot::default(),
            conditions: ReportedConditions::Unreported,
            battery: ObservedBattery::NotObserved,
            profile_models: DrainModelLearning::NotLoaded,
            estimates: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum PowerProfileMessage {
    Status(SnapshotDelivery<ProfileStatus>),
    Conditions(SnapshotDelivery<Conditions>),
    Loaded(HashMap<Profile, RegressionState>),
    ControlCommitted(ControlMessage),
}

#[derive(Debug, Clone)]
pub enum PowerProfileInteraction {
    Control(ControlMessage),
    SwitchToProfile(Profile),
}

fn commit_profile_switch(profile: Profile) -> Task<PowerProfileMessage> {
    ControlMessage::commit(
        "power profile set",
        profile.position(),
        power_profile_selection::set(profile),
    )
    .map(PowerProfileMessage::ControlCommitted)
}

struct DrainSample {
    profile: Profile,
    conditions: Conditions,
    draw: Power,
}

impl PowerProfileControl {
    pub fn close(&mut self) {
        self.control.close();
    }

    pub fn subscription(buses: &DesktopBuses) -> Subscription<PowerProfileMessage> {
        Subscription::batch([
            snapshot_subscription::<ProfileStatus>(buses.clone()).map(PowerProfileMessage::Status),
            snapshot_subscription::<Conditions>(buses.clone()).map(PowerProfileMessage::Conditions),
        ])
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<PowerProfileMessage> {
        Task::batch([
            first_delivery::<ProfileStatus>(buses.clone()).map(PowerProfileMessage::Status),
            first_delivery::<Conditions>(buses.clone()).map(PowerProfileMessage::Conditions),
        ])
    }

    pub fn update(
        &mut self,
        profile_message: PowerProfileMessage,
        clock: Clock,
    ) -> Task<PowerProfileMessage> {
        match profile_message {
            PowerProfileMessage::Status(delivery) => {
                self.status.receive(delivery, clock);
                if let Some(profile) = self.active_profile() {
                    self.control
                        .update(ControlMessage::Sample(profile.position()));
                }
                self.recompute();
            }
            PowerProfileMessage::Conditions(delivery) => {
                self.conditions = match delivery {
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Ok(conditions),
                        ..
                    } => ReportedConditions::Reported(conditions),
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Err(error),
                        ..
                    } => {
                        tracing::debug!("power conditions: {error}");
                        ReportedConditions::Unreported
                    }
                    SnapshotDelivery::SourceUnavailable => ReportedConditions::Unreported,
                    SnapshotDelivery::SnapshotUnchanged => self.conditions,
                };
            }
            PowerProfileMessage::Loaded(models) => {
                self.profile_models = DrainModelLearning::Learning {
                    models,
                    spacing: SampleSpacing::Interrupted,
                };
                self.recompute();
            }
            PowerProfileMessage::ControlCommitted(control_message) => {
                return self.update_control(control_message);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: PowerProfileInteraction) -> Task<PowerProfileMessage> {
        match interaction {
            PowerProfileInteraction::Control(control_message) => {
                self.update_control(control_message)
            }
            PowerProfileInteraction::SwitchToProfile(profile) => commit_profile_switch(profile),
        }
    }

    fn update_control(&mut self, control_message: ControlMessage) -> Task<PowerProfileMessage> {
        match self.control.update(control_message) {
            Action::Commit(value) => {
                commit_profile_switch(Profile::nearest_position_profile(value))
            }
            Action::Opened | Action::None | Action::Right | Action::Adjust(_) => Task::none(),
        }
    }

    fn active_profile(&self) -> Option<Profile> {
        self.status.get().map(|status| status.active)
    }

    pub fn profiles_saving_more_power(&self) -> impl Iterator<Item = Profile> {
        let active_profile = self.active_profile();
        Profile::ALL.into_iter().filter(move |profile| {
            active_profile.is_some_and(|active| profile.scale() < active.scale())
        })
    }

    pub fn observe_battery(
        &mut self,
        battery: &Battery,
        now: Instant,
    ) -> Task<PowerProfileMessage> {
        let task = self.learn(battery, now);
        self.battery = ObservedBattery::Observed(battery.clone());
        self.recompute();
        task
    }

    fn drain_sample(&self, battery: &Battery) -> Option<DrainSample> {
        let measurable = battery.status == ChargeStatus::Discharging
            && battery.power > Power::new::<watt>(MIN_SAMPLE_W);
        let ReportedConditions::Reported(conditions) = self.conditions else {
            return None;
        };
        Some(DrainSample {
            profile: self.active_profile()?,
            conditions,
            draw: measurable.then_some(battery.power)?,
        })
    }

    fn learn(&mut self, battery: &Battery, now: Instant) -> Task<PowerProfileMessage> {
        let sample = self.drain_sample(battery);
        let DrainModelLearning::Learning { models, spacing } = &mut self.profile_models else {
            return self.start_loading(&battery.serial);
        };
        let Some(sample) = sample else {
            *spacing = SampleSpacing::Interrupted;
            return Task::none();
        };
        let since_previous = spacing.sampled_now(now);
        let next = models
            .remove(&sample.profile)
            .unwrap_or_else(|| RegressionState::seed(sample.draw))
            .observe(sample.conditions, sample.draw, since_previous);
        models.insert(sample.profile, next);
        Task::future(sqlite_power_draw_regression::save(
            battery.serial.clone(),
            sample.profile,
            next,
        ))
        .discard()
    }

    fn start_loading(&mut self, serial: &str) -> Task<PowerProfileMessage> {
        if matches!(self.profile_models, DrainModelLearning::Loading) || serial.is_empty() {
            return Task::none();
        }
        self.profile_models = DrainModelLearning::Loading;
        Task::future(sqlite_power_draw_regression::load(serial.to_string()))
            .map(PowerProfileMessage::Loaded)
    }

    fn drain_estimates(&self) -> Option<HashMap<Profile, Option<Estimate>>> {
        let ObservedBattery::Observed(battery) = &self.battery else {
            return None;
        };
        let ReportedConditions::Reported(conditions) = self.conditions else {
            return None;
        };
        let DrainModelLearning::Learning { models, .. } = &self.profile_models else {
            return None;
        };
        if battery.status != ChargeStatus::Discharging {
            return None;
        }
        Some(time_until_empty(
            battery,
            self.active_profile()?,
            conditions,
            models,
        ))
    }

    fn recompute(&mut self) {
        self.estimates = self.drain_estimates().unwrap_or_default();
    }

    pub fn expected_draw(&self) -> LaptopDrawEstimate {
        let DrainModelLearning::Learning { models, .. } = &self.profile_models else {
            return LaptopDrawEstimate::NotModelled;
        };
        let ReportedConditions::Reported(conditions) = self.conditions else {
            return LaptopDrawEstimate::NotModelled;
        };
        let Some(model) = self
            .active_profile()
            .and_then(|active_profile| models.get(&active_profile))
        else {
            return LaptopDrawEstimate::NotModelled;
        };
        let expected_draw = model.expected_draw(conditions);
        if expected_draw > Power::default() {
            LaptopDrawEstimate::Modelled(expected_draw)
        } else {
            LaptopDrawEstimate::NotModelled
        }
    }

    pub fn active_estimate(&self) -> Option<Estimate> {
        self.estimates
            .get(&self.active_profile()?)
            .copied()
            .flatten()
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let profile = Section::new(
            SectionTitle::captioned(
                "Power profile",
                "Lower profiles trade speed for less power draw, heat and fan noise.",
            ),
            dictionary_table([LabeledValue::new("Active", self.control.readout_cell())]),
        );
        let battery_life = (!self.estimates.is_empty()).then(|| self.battery_life_section());
        let mut sections = self
            .status
            .hover_card_with(once(profile).chain(battery_life).collect());
        sections.extend(self.status.sections_described_by(profile_status_sections));
        sections
    }

    fn battery_life_section(&self) -> Section {
        let active_profile = Profile::nearest_position_profile(
            self.control.expanding_slider.synced_value.shown_value(),
        );
        let lives = Profile::ALL.map(|profile| ProfileLife {
            profile,
            activity: if profile == active_profile {
                ProfileActivity::Active
            } else {
                ProfileActivity::Inactive
            },
            forecast: Forecast::estimated_or(
                self.estimates.get(&profile).copied().flatten(),
                Unforecastable::NoDrainHistory,
            ),
        });
        Section::new(
            SectionTitle::captioned(
                "Battery life",
                "Time until empty if you switch to each profile now.",
            ),
            headed_table(ESTIMATE_HEADINGS, lives),
        )
    }
}

struct ProfileLife {
    profile: Profile,
    activity: ProfileActivity,
    forecast: Forecast,
}

impl ProfileLife {
    fn title(&self) -> String {
        match self.activity {
            ProfileActivity::Active => format!("{} (active)", self.profile.title()),
            ProfileActivity::Inactive => self.profile.title().to_string(),
        }
    }
}

impl From<ProfileLife> for TableRecord<3> {
    fn from(profile_life: ProfileLife) -> Self {
        TableRecord::with_cells(profile_life.title(), profile_life.forecast.cells())
    }
}
