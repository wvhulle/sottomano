use std::time::Duration;

use chrono::{DurationRound, TimeDelta};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    weather::{
        buienradar_rain::precipitation::millimeter_per_hour,
        open_meteo_pollen::{Allergen, AllergenConcentration, Pollen},
        open_meteo_precipitation_model::{ModelQuarterHour, PrecipitationModel},
        open_meteo_temperature::{
            Daylight, HourlyForecast, HourlyTemperature, Intensity, SkyCondition, Temperature,
            WeatherCondition,
        },
        open_meteo_wind::Wind,
    },
};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::weather_group::{WeatherGroup, WeatherMessage},
};
use uom::si::{
    angle::degree,
    f64::{Angle, Ratio, ThermodynamicTemperature, Velocity},
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
    velocity::kilometer_per_hour,
};

const QUARTER_HOUR_MINUTES: i64 = 15;
const MODEL_QUARTER_HOURS: [(f64, WeatherCondition); 6] = [
    (0.0, WeatherCondition::PartlyCloudy),
    (0.0, WeatherCondition::PartlyCloudy),
    (0.3, WeatherCondition::Drizzle(Intensity::Light)),
    (0.6, WeatherCondition::Drizzle(Intensity::Light)),
    (0.0, WeatherCondition::PartlyCloudy),
    (0.0, WeatherCondition::MainlyClear),
];
const POLLEN_GRAINS_PER_CUBIC_METRE: [(Allergen, f64); 6] = [
    (Allergen::Alder, 0.0),
    (Allergen::Birch, 0.0),
    (Allergen::Grass, 42.0),
    (Allergen::Mugwort, 3.0),
    (Allergen::Olive, 18.0),
    (Allergen::Ragweed, 0.0),
];

use crate::{fixtures::location::porto_location, showcased_group::ShowcasedGroup};

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::Weather(Box::new(fixture_messages(clock).into_iter().fold(
        WeatherGroup::default(),
        |mut group, fixture_message| {
            let _ = group.update(fixture_message, clock);
            group
        },
    )))
}

fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

fn hourly(
    hours_from_now: i64,
    degrees: f64,
    condition: WeatherCondition,
    daylight: Daylight,
    clock: Clock,
) -> HourlyTemperature {
    HourlyTemperature {
        time: clock
            .utc()
            .duration_trunc(TimeDelta::hours(1))
            .expect("the current hour is representable")
            + TimeDelta::hours(hours_from_now),
        temp: celsius(degrees),
        condition: SkyCondition::Recognized(condition),
        daylight,
    }
}

fn model_quarter_hours(clock: Clock) -> Vec<ModelQuarterHour> {
    let current_quarter = clock
        .utc()
        .duration_trunc(TimeDelta::minutes(QUARTER_HOUR_MINUTES))
        .expect("the current quarter hour is representable");
    MODEL_QUARTER_HOURS
        .iter()
        .zip(0..)
        .map(
            |(&(millimeters_per_hour, condition), quarters_from_now)| ModelQuarterHour {
                starts_at: current_quarter
                    + TimeDelta::minutes(QUARTER_HOUR_MINUTES * quarters_from_now),
                intensity: Velocity::new::<millimeter_per_hour>(millimeters_per_hour),
                condition,
                daylight: Daylight::Day,
            },
        )
        .collect()
}

fn precipitation_model(clock: Clock) -> PrecipitationModel {
    let quarters = model_quarter_hours(clock);
    PrecipitationModel {
        current: quarters[0].clone(),
        quarters,
    }
}

fn pollen_counts() -> Pollen {
    Pollen {
        concentrations: POLLEN_GRAINS_PER_CUBIC_METRE
            .into_iter()
            .map(|(allergen, grains_per_cubic_metre)| AllergenConcentration {
                allergen,
                grains_per_cubic_metre,
            })
            .collect(),
    }
}

fn fixture_messages(clock: Clock) -> [WeatherMessage; 5] {
    [
        WeatherMessage::Location(SnapshotDelivery::from(porto_location(clock))),
        WeatherMessage::Wind(SnapshotDelivery::fixture_delivered(
            Ok(Wind {
                speed: Velocity::new::<kilometer_per_hour>(18.0),
                gusts: Velocity::new::<kilometer_per_hour>(31.0),
                direction: Angle::new::<degree>(320.0),
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(300))),
        )),
        WeatherMessage::PrecipitationModel(SnapshotDelivery::fixture_delivered(
            Ok(precipitation_model(clock)),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_mins(15))),
        )),
        WeatherMessage::Pollen(SnapshotDelivery::fixture_delivered(
            Ok(pollen_counts()),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_mins(30))),
        )),
        WeatherMessage::Temperature(SnapshotDelivery::fixture_delivered(
            Ok(Temperature {
                condition: SkyCondition::Recognized(WeatherCondition::PartlyCloudy),
                daylight: Daylight::Day,
                temp: celsius(18.0),
                feels: celsius(17.0),
                temp_min: celsius(12.0),
                temp_max: celsius(21.0),
                humidity: Ratio::new::<percent>(64.0),
                hourly: HourlyForecast {
                    earlier: vec![
                        hourly(-2, 15.0, WeatherCondition::Overcast, Daylight::Day, clock),
                        hourly(
                            -1,
                            17.0,
                            WeatherCondition::PartlyCloudy,
                            Daylight::Day,
                            clock,
                        ),
                    ],
                    later: vec![
                        hourly(1, 19.0, WeatherCondition::MainlyClear, Daylight::Day, clock),
                        hourly(
                            2,
                            16.0,
                            WeatherCondition::PartlyCloudy,
                            Daylight::Day,
                            clock,
                        ),
                        hourly(3, 14.0, WeatherCondition::Clear, Daylight::Day, clock),
                    ],
                },
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(300))),
        )),
    ]
}
