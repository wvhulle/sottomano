use chrono::{DurationRound, TimeDelta, Utc};
use situo_client::{Estimate, Fix, Method, Observations, Origin, Place, Position};
use uom::si::{
    f64::{Ratio, ThermodynamicTemperature},
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    data::source::{
        Delivery,
        weather::{HourlyForecast, HourlyTemperature, Location, Temperature, WeatherCondition},
    },
    readme_showcase::showcased_group::ShowcasedGroup,
    widget_groups::weather_group::{WeatherGroup, WeatherMessage},
};

pub fn group() -> ShowcasedGroup {
    ShowcasedGroup::Weather(Box::new(readings().into_iter().fold(
        WeatherGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading);
            group
        },
    )))
}

fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

fn hourly(hours_from_now: i64, degrees: f64, condition: WeatherCondition) -> HourlyTemperature {
    HourlyTemperature {
        time: Utc::now()
            .duration_trunc(TimeDelta::hours(1))
            .expect("the current hour is representable")
            + TimeDelta::hours(hours_from_now),
        temp: celsius(degrees),
        condition: Some(condition),
    }
}

fn readings() -> [WeatherMessage; 2] {
    [
        WeatherMessage::Temp(Delivery::Reading(Ok(Temperature {
            condition: Some(WeatherCondition::PartlyCloudy),
            temp: celsius(18.0),
            feels: celsius(17.0),
            temp_min: celsius(12.0),
            temp_max: celsius(21.0),
            humidity: Ratio::new::<percent>(64.0),
            hourly: HourlyForecast {
                earlier: vec![
                    hourly(-2, 15.0, WeatherCondition::Overcast),
                    hourly(-1, 17.0, WeatherCondition::PartlyCloudy),
                ],
                later: vec![
                    hourly(1, 19.0, WeatherCondition::MainlyClear),
                    hourly(2, 20.0, WeatherCondition::Clear),
                    hourly(3, 21.0, WeatherCondition::Clear),
                ],
            },
        }))),
        WeatherMessage::Location(Delivery::Reading(Ok(Location {
            geo: Fix {
                position: Position {
                    latitude: 51.0533,
                    longitude: 3.7211,
                },
                origin: Origin::Estimated(Estimate {
                    method: Method::Wifi,
                    accuracy_metres: 305.0,
                    observations: Observations {
                        access_points_seen: 17,
                        access_points_submitted: 15,
                        cells_seen: 1,
                        cells_submitted: 1,
                    },
                }),
                place: Some(Place {
                    name: "Gent".to_owned(),
                    country: "BE".to_owned(),
                    distance_km: 0.48,
                }),
                timestamp: Utc::now().timestamp(),
            },
        }))),
    ]
}
