use std::time::Duration;

use chrono::{DurationRound, TimeDelta};
use jiff::Timestamp;
use situo_client::{
    BeaconDbEstimate, EstimateMethod, FixOrigin, GeoCoordinates, LocationFix, NearestPlace,
    ObservationCounts,
};
use sottomano_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    weather::{
        open_meteo_temperature::{
            Daylight, HourlyForecast, HourlyTemperature, Temperature, WeatherCondition,
        },
        situo_location_fix::Location,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::weather_group::{WeatherGroup, WeatherMessage},
};
use uom::si::{
    angle::degree,
    f64::{Angle, Length, Ratio, ThermodynamicTemperature},
    length::{kilometer, meter},
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::Weather(Box::new(readings(clock).into_iter().fold(
        WeatherGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading, clock);
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
        condition: Some(condition),
        daylight,
    }
}

fn readings(clock: Clock) -> [WeatherMessage; 2] {
    [
        WeatherMessage::Temp(SnapshotDelivery::fixture_delivered(
            Ok(Temperature {
                condition: Some(WeatherCondition::PartlyCloudy),
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
                            Daylight::Night,
                            clock,
                        ),
                        hourly(3, 14.0, WeatherCondition::Clear, Daylight::Night, clock),
                    ],
                },
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(300))),
        )),
        WeatherMessage::Location(SnapshotDelivery::live_push(Ok(Location {
            geo: LocationFix {
                position: GeoCoordinates {
                    latitude: Angle::new::<degree>(51.0533),
                    longitude: Angle::new::<degree>(3.7211),
                },
                origin: FixOrigin::Estimated(BeaconDbEstimate {
                    method: EstimateMethod::Wifi,
                    accuracy: Length::new::<meter>(305.0),
                    observations: ObservationCounts {
                        access_points_seen: 17,
                        access_points_submitted: 15,
                        cells_seen: 1,
                        cells_submitted: 1,
                    },
                }),
                place: Some(NearestPlace {
                    name: "Gent".to_owned(),
                    country: "BE".to_owned(),
                    distance: Length::new::<kilometer>(0.48),
                }),
                timestamp: Timestamp::from_second(clock.utc().timestamp())
                    .expect("the showcase clock is within jiff's range"),
            },
        }))),
    ]
}
