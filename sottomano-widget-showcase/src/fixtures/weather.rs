use std::time::Duration;

use chrono::{DurationRound, TimeDelta};
use jiff::Timestamp;
use situo_client::{
    AccessPointExclusions, BeaconDbEstimate, CellRadio, DecidingSource, FixOrigin, GeoCoordinates,
    LocationFix, NearestPlace, ObservationCounts, PositionEstimate, RadioSource, ServingCell,
    SourceLookup, SourceLookupOutcome, UpdateSchedule, UpdateTrigger,
};
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
        situo_location_fix::Location,
    },
};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::weather_group::{WeatherGroup, WeatherMessage},
};
use uom::si::{
    angle::degree,
    f64::{Angle, Length, Ratio, ThermodynamicTemperature, Velocity},
    length::{kilometer, meter},
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

use crate::showcased_group::ShowcasedGroup;

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
        WeatherMessage::Location(SnapshotDelivery::from(Location {
            geo: porto_location_fix(clock),
            update_schedule: UpdateSchedule::from_wire(
                900,
                30,
                [
                    UpdateTrigger::ServingCellChange,
                    UpdateTrigger::WifiNeighbourhoodChange,
                    UpdateTrigger::NetworkReconnect,
                    UpdateTrigger::ModemHotplug,
                ],
            ),
        })),
    ]
}

fn porto_location_fix(clock: Clock) -> LocationFix {
    LocationFix {
        position: GeoCoordinates {
            latitude: Angle::new::<degree>(41.1496),
            longitude: Angle::new::<degree>(-8.6110),
        },
        origin: FixOrigin::Estimated(BeaconDbEstimate {
            decided_by: DecidingSource::Radio(RadioSource::Wifi),
            accuracy: Length::new::<meter>(305.0),
            source_lookups: vec![
                SourceLookup {
                    source: RadioSource::Wifi,
                    outcome: SourceLookupOutcome::Located(PositionEstimate {
                        position: GeoCoordinates {
                            latitude: Angle::new::<degree>(41.1496),
                            longitude: Angle::new::<degree>(-8.6110),
                        },
                        accuracy: Length::new::<meter>(305.0),
                    }),
                },
                SourceLookup {
                    source: RadioSource::Cell,
                    outcome: SourceLookupOutcome::Located(PositionEstimate {
                        position: GeoCoordinates {
                            latitude: Angle::new::<degree>(41.1532),
                            longitude: Angle::new::<degree>(-8.6051),
                        },
                        accuracy: Length::new::<meter>(1_250.0),
                    }),
                },
            ],
            serving_cells: vec![ServingCell {
                radio: CellRadio::Lte,
                mobile_country_code: 268,
                mobile_network_code: 1,
                location_area_code: 0x1F4A,
                cell_id: 0x0310_2C05,
            }],
            observations: ObservationCounts {
                access_points_seen: 17,
                access_points_submitted: 15,
                access_point_exclusions: AccessPointExclusions {
                    opted_out: 0,
                    hidden: 1,
                    mobile_hotspot: 1,
                    below_lookup_minimum: 0,
                },
                cells_seen: 1,
                cells_submitted: 1,
            },
        }),
        place: Some(NearestPlace {
            name: "Porto".to_owned(),
            country: "PT".to_owned(),
            distance: Length::new::<kilometer>(0.48),
        }),
        timestamp: Timestamp::from_second(clock.utc().timestamp())
            .expect("the showcase clock is within jiff's range"),
    }
}
