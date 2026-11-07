use std::{
    collections::HashMap,
    ops::RangeInclusive,
    time::{Duration, Instant},
};

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
    group_items::slider_control::{Action, ControlMessage, SliderControl},
    hover_cards::hover_card_body::{HoverCardBody, Section, hover_card_source_delivery::HoverCard},
    widget_groups::power_group::{
        PowerTarget,
        estimate_columns::{ESTIMATE_HEADINGS, Forecast, Unforecastable},
        sqlite_power_draw_regression,
    },
    widgets::{
        keyed_record_table::{IntoTableRecord, TableRecord},
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
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
    control: SliderControl,
    status: LatestSnapshot<ProfileStatus, HidesFetchFailures>,
    conditions: Option<Conditions>,
    battery: Option<Battery>,
    profile_models: Option<HashMap<Profile, RegressionState>>,
    estimates: HashMap<Profile, Option<Estimate>>,
    load_started: bool,
    previous_sample_at: Option<Instant>,
}

impl Default for PowerProfileControl {
    fn default() -> Self {
        Self {
            control: SliderControl::new(
                "Power profile",
                RANGE,
                PowerTarget::Profile,
                profile_face(Profile::Balanced),
            )
            .slider_step(STEP)
            .scroll_step(STEP)
            .icon_only()
            .readout(|value, _| Profile::from_position(value).title().to_string())
            .glyph(|value, _| profile_face(Profile::from_position(value))),
            status: LatestSnapshot::default(),
            conditions: None,
            battery: None,
            profile_models: None,
            estimates: HashMap::new(),
            load_started: false,
            previous_sample_at: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PowerProfileMessage {
    Status(SnapshotDelivery<ProfileStatus>),
    Conditions(SnapshotDelivery<Conditions>),
    Loaded(HashMap<Profile, RegressionState>),
    Control(ControlMessage),
    SwitchToProfile(Profile),
}

fn commit_profile_switch(profile: Profile) -> Task<PowerProfileMessage> {
    ControlMessage::commit("power profile set", profile.position(), move || {
        power_profile_selection::set(profile)
    })
    .map(PowerProfileMessage::Control)
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

    pub fn update(&mut self, msg: PowerProfileMessage, clock: Clock) -> Task<PowerProfileMessage> {
        match msg {
            PowerProfileMessage::Status(delivery) => {
                self.status.receive(delivery, clock);
                if let Some(profile) = self.active_profile() {
                    self.control
                        .update(ControlMessage::Reading(profile.position()));
                }
                self.recompute();
            }
            PowerProfileMessage::Conditions(delivery) => {
                self.conditions = match delivery {
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Ok(conditions),
                        ..
                    } => Some(conditions),
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Err(error),
                        ..
                    } => {
                        tracing::debug!("power conditions: {error}");
                        None
                    }
                    SnapshotDelivery::SourceUnavailable
                    | SnapshotDelivery::InternetUnreachable(_) => None,
                    SnapshotDelivery::SnapshotUnchanged => self.conditions,
                };
            }
            PowerProfileMessage::Loaded(models) => {
                self.profile_models = Some(models);
                self.recompute();
            }
            PowerProfileMessage::Control(msg) => {
                return match self.control.update(msg) {
                    Action::Commit(value) => commit_profile_switch(Profile::from_position(value)),
                    Action::Opened | Action::None | Action::Right | Action::Adjust(_) => {
                        Task::none()
                    }
                };
            }
            PowerProfileMessage::SwitchToProfile(profile) => return commit_profile_switch(profile),
        }
        Task::none()
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
        self.battery = Some(battery.clone());
        self.recompute();
        task
    }

    fn drain_sample(&self, battery: &Battery) -> Option<DrainSample> {
        let measurable = battery.status == ChargeStatus::Discharging
            && battery.power > Power::new::<watt>(MIN_SAMPLE_W);
        Some(DrainSample {
            profile: self.active_profile()?,
            conditions: self.conditions?,
            draw: measurable.then_some(battery.power)?,
        })
    }

    fn learn(&mut self, battery: &Battery, now: Instant) -> Task<PowerProfileMessage> {
        let sample = self.drain_sample(battery);
        let Some(models) = self.profile_models.as_mut() else {
            return self.start_loading(&battery.serial);
        };
        let Some(sample) = sample else {
            self.previous_sample_at = None;
            return Task::none();
        };
        let since_previous = self
            .previous_sample_at
            .replace(now)
            .map_or(Duration::ZERO, |previous| now - previous);
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
        if self.load_started || serial.is_empty() {
            return Task::none();
        }
        self.load_started = true;
        Task::future(sqlite_power_draw_regression::load(serial.to_string()))
            .map(PowerProfileMessage::Loaded)
    }

    fn drain_estimates(&self) -> Option<HashMap<Profile, Option<Estimate>>> {
        let battery = self
            .battery
            .as_ref()
            .filter(|battery| battery.status == ChargeStatus::Discharging)?;
        Some(time_until_empty(
            battery,
            self.active_profile()?,
            self.conditions?,
            self.profile_models.as_ref()?,
        ))
    }

    fn recompute(&mut self) {
        self.estimates = self.drain_estimates().unwrap_or_default();
    }

    pub fn expected_draw(&self) -> Option<Power> {
        let model = self.profile_models.as_ref()?.get(&self.active_profile()?)?;
        Some(model.expected_draw(self.conditions?)).filter(|draw| *draw > Power::default())
    }

    pub fn active_estimate(&self) -> Option<Estimate> {
        self.estimates
            .get(&self.active_profile()?)
            .copied()
            .flatten()
    }

    pub fn hover_card(&self, clock: Clock) -> Option<HoverCard> {
        let profile =
            Section::dictionary([LabeledValue::new("Active", self.control.readout_cell())]).titled(
                (
                    "Power profile",
                    "Lower profiles trade speed for less power draw, heat and fan noise.",
                ),
            );
        let battery_life = (!self.estimates.is_empty()).then(|| self.battery_life_section());
        self.status
            .hover_card_with(HoverCardBody::sections(
                [Some(profile), battery_life].into_iter().flatten(),
            ))
            .map(|hover_card| hover_card.with_groups(self.status.sections(clock)))
    }

    fn battery_life_section(&self) -> Section {
        let active_profile = Profile::from_position(self.control.current());
        let lives = Profile::ALL.map(|profile| ProfileLife {
            profile,
            active: profile == active_profile,
            forecast: Forecast::from_estimate(
                self.estimates.get(&profile).copied().flatten(),
                Unforecastable::NoDrainHistory,
            ),
        });
        Section::column_table(ESTIMATE_HEADINGS, lives).titled((
            "Battery life",
            "Time until empty if you switch to each profile now.",
        ))
    }

    pub fn slider(&self) -> Option<&SliderControl> {
        self.status.get().map(|_| &self.control)
    }
}

struct ProfileLife {
    profile: Profile,
    active: bool,
    forecast: Forecast,
}

impl ProfileLife {
    fn title(&self) -> String {
        if self.active {
            format!("{} (active)", self.profile.title())
        } else {
            self.profile.title().to_string()
        }
    }
}

impl IntoTableRecord<3> for ProfileLife {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(self.title(), self.forecast.cells())
    }
}
