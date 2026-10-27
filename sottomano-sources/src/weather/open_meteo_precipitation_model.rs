use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use futures::Stream;
use serde::Deserialize;
use situo_client::GeoCoordinates;
use uom::si::f64::Velocity;

use super::{
    Error,
    buienradar_rain::precipitation::millimeter_per_hour,
    open_meteo_temperature::{Daylight, WeatherCondition, open_meteo_time, open_meteo_times},
};
use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::http_json_client,
};

const QUARTER_HOUR: TimeDelta = TimeDelta::minutes(15);

const QUARTER_HOURS_PER_HOUR: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PrecipitationModel {
    pub current: ModelQuarterHour,
    pub quarters: Vec<ModelQuarterHour>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelQuarterHour {
    pub starts_at: DateTime<Utc>,
    pub intensity: Velocity,
    pub condition: WeatherCondition,
    pub daylight: Daylight,
}

impl ModelQuarterHour {
    fn ending_at(
        ends_at: DateTime<Utc>,
        millimeters_in_quarter: f64,
        condition: WeatherCondition,
        daylight: Daylight,
    ) -> Self {
        ModelQuarterHour {
            starts_at: ends_at - QUARTER_HOUR,
            intensity: Velocity::new::<millimeter_per_hour>(
                millimeters_in_quarter * QUARTER_HOURS_PER_HOUR,
            ),
            condition,
            daylight,
        }
    }
}

#[derive(Deserialize)]
struct ModelResponse {
    current: CurrentQuarterHour,
    minutely_15: QuarterHourSeries,
}

#[derive(Deserialize)]
struct CurrentQuarterHour {
    #[serde(deserialize_with = "open_meteo_time")]
    time: DateTime<Utc>,
    precipitation: f64,
    weather_code: WeatherCondition,
    is_day: Daylight,
}

#[derive(Deserialize)]
struct QuarterHourSeries {
    #[serde(deserialize_with = "open_meteo_times")]
    time: Vec<DateTime<Utc>>,
    precipitation: Vec<f64>,
    weather_code: Vec<WeatherCondition>,
    is_day: Vec<Daylight>,
}

impl From<ModelResponse> for PrecipitationModel {
    fn from(response: ModelResponse) -> Self {
        let current = response.current;
        let series = response.minutely_15;
        PrecipitationModel {
            current: ModelQuarterHour::ending_at(
                current.time,
                current.precipitation,
                current.weather_code,
                current.is_day,
            ),
            quarters: series
                .time
                .into_iter()
                .zip(series.precipitation)
                .zip(series.weather_code)
                .zip(series.is_day)
                .map(|(((ends_at, millimeters), condition), daylight)| {
                    ModelQuarterHour::ending_at(ends_at, millimeters, condition, daylight)
                })
                .collect(),
        }
    }
}

impl SnapshotSource for PrecipitationModel {
    type Error = Error;
    type StreamInputs = GeoCoordinates;

    const NAME: &'static str = "Precipitation model";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_mins(15)).poll_deliveries(move || Self::poll(inputs))
    }
}

impl PrecipitationModel {
    async fn poll(here: GeoCoordinates) -> Result<Self, Error> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=precipitation,weather_code,is_day&minutely_15=precipitation,weather_code,is_day&forecast_minutely_15=13&timezone=auto&timeformat=unixtime",
            here.latitude_degrees(),
            here.longitude_degrees()
        );
        let response: ModelResponse = http_json_client::get_json(&url, &[], 10).await?;
        Ok(response.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::open_meteo_temperature::Intensity;

    fn decoded_model(weather_codes: &str) -> serde_json::Result<PrecipitationModel> {
        serde_json::from_str::<ModelResponse>(&format!(
            r#"{{
                "current": {{
                    "time": 1792761300,
                    "interval": 900,
                    "precipitation": 1.4,
                    "weather_code": 63,
                    "is_day": 1
                }},
                "minutely_15": {{
                    "time": [1792761300, 1792762200],
                    "precipitation": [1.4, 0.2],
                    "weather_code": {weather_codes},
                    "is_day": [1, 0]
                }}
            }}"#
        ))
        .map(PrecipitationModel::from)
    }

    #[test]
    fn quarter_hours_start_fifteen_minutes_before_their_stamp() {
        let model = decoded_model("[63, 53]").unwrap();
        assert_eq!(
            model.quarters[0].starts_at,
            "2026-10-23T13:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(model.current.starts_at, model.quarters[0].starts_at);
    }

    #[test]
    fn quarter_hour_sums_become_hourly_rates() {
        let model = decoded_model("[63, 53]").unwrap();
        let rate_text = |quarter: &ModelQuarterHour| {
            format!("{:.1}", quarter.intensity.get::<millimeter_per_hour>())
        };
        assert_eq!(rate_text(&model.current), "5.6");
        assert_eq!(rate_text(&model.quarters[1]), "0.8");
    }

    #[test]
    fn conditions_and_daylight_decode_per_quarter() {
        let model = decoded_model("[63, 53]").unwrap();
        assert_eq!(
            model.quarters[1].condition,
            WeatherCondition::Drizzle(Intensity::Moderate)
        );
        assert_eq!(model.quarters[1].daylight, Daylight::Night);
    }

    #[test]
    fn an_undocumented_weather_code_rejects_the_response() {
        assert!(decoded_model("[63, 42]").is_err());
    }
}
