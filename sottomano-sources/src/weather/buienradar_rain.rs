use std::time::Duration;

use chrono::{DateTime, Local, NaiveDateTime};
use futures::Stream;
use serde::Deserialize;
use situo_client::Position;
use uom::si::f64::Velocity;

use super::Error;
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, polled_delivery_stream,
    system_protocol_clients::http_json_client,
};

pub mod precipitation {
    use uom::{
        Conversion,
        si::{Unit, velocity},
    };

    #[allow(non_camel_case_types)]
    #[derive(Clone, Copy, Debug, Hash)]
    pub struct millimeter_per_hour;

    impl Unit for millimeter_per_hour {
        fn abbreviation() -> &'static str {
            "mm/h"
        }

        fn singular() -> &'static str {
            "millimeter per hour"
        }

        fn plural() -> &'static str {
            "millimeters per hour"
        }
    }

    impl velocity::Unit for millimeter_per_hour {}

    impl Conversion<f64> for millimeter_per_hour {
        type T = f64;

        fn coefficient() -> f64 {
            1.0 / 3_600_000.0
        }
    }

    impl velocity::Conversion<f64> for millimeter_per_hour {}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rain {
    pub intensity: Velocity,
    pub peak: Downpour,
    pub outlook: RainOutlook,
    pub forecast: Vec<Downpour>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Downpour {
    pub intensity: Velocity,
    pub at: DateTime<Local>,
}

impl Downpour {
    #[must_use]
    pub fn exceeds(&self, threshold: Velocity) -> bool {
        self.intensity > threshold
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RainOutlook {
    Dry,
    StartsAt(DateTime<Local>),
    StopsAt(DateTime<Local>),
    OutlastsForecastUntil(DateTime<Local>),
}

impl RainOutlook {
    fn of(forecast: &[Downpour]) -> Self {
        Self::of_rain_above(forecast, Velocity::default())
    }

    #[must_use]
    pub fn of_rain_above(forecast: &[Downpour], threshold: Velocity) -> Self {
        let Some((now, later)) = forecast.split_first() else {
            return RainOutlook::Dry;
        };
        if now.exceeds(threshold) {
            Self::while_raining(now, later, threshold)
        } else {
            Self::while_dry(later, threshold)
        }
    }

    fn while_dry(later: &[Downpour], threshold: Velocity) -> Self {
        later
            .iter()
            .find(|downpour| downpour.exceeds(threshold))
            .map_or(RainOutlook::Dry, |first_wet| {
                RainOutlook::StartsAt(first_wet.at)
            })
    }

    fn while_raining(now: &Downpour, later: &[Downpour], threshold: Velocity) -> Self {
        later
            .iter()
            .find(|downpour| !downpour.exceeds(threshold))
            .map_or_else(
                || RainOutlook::OutlastsForecastUntil(later.last().unwrap_or(now).at),
                |first_dry| RainOutlook::StopsAt(first_dry.at),
            )
    }
}

#[derive(Deserialize)]
struct RadarNowcast {
    forecasts: Vec<RadarSlot>,
}

#[derive(Deserialize)]
struct RadarSlot {
    utcdatetime: NaiveDateTime,
    precipitation: f64,
}

impl From<RadarSlot> for Downpour {
    fn from(slot: RadarSlot) -> Self {
        Downpour {
            intensity: Velocity::new::<precipitation::millimeter_per_hour>(slot.precipitation),
            at: slot.utcdatetime.and_utc().with_timezone(&Local),
        }
    }
}

impl ReadingSource for Rain {
    type Error = Error;
    type StreamInputs = Position;

    const NAME: &'static str = "weather-rain";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(60));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl Rain {
    async fn poll(here: Position) -> Result<Self, Error> {
        let url = format!(
            "https://graphdata.buienradar.nl/2.0/forecast/geo/Rain3Hour?lat={}&lon={}",
            here.latitude, here.longitude
        );
        let nowcast: RadarNowcast =
            http_json_client::get_json(&url, &[], 5)
                .await
                .map_err(|error| match error {
                    http_json_client::Error::Status { status: 404, .. } => Error::OutsideRainRadar,
                    other => Error::from(other),
                })?;
        Self::from_forecast(nowcast.forecasts.into_iter().map(Downpour::from).collect())
            .ok_or(Error::Malformed)
    }

    #[must_use]
    pub fn from_forecast(forecast: Vec<Downpour>) -> Option<Self> {
        let peak = forecast
            .iter()
            .max_by(|earlier, later| earlier.intensity.value.total_cmp(&later.intensity.value))
            .cloned()?;
        Some(Rain {
            intensity: forecast.first()?.intensity,
            peak,
            outlook: RainOutlook::of(&forecast),
            forecast,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeDelta, TimeZone, Utc};
    use serde_json::json;

    use super::*;

    fn september_27_utc(hour: u32, minute: u32) -> DateTime<Local> {
        Utc.with_ymd_and_hms(2026, 9, 27, hour, minute, 0)
            .unwrap()
            .with_timezone(&Local)
    }

    fn decoded_forecast(slots: &[(&str, f64)]) -> Vec<Downpour> {
        let forecasts: Vec<_> = slots
            .iter()
            .map(|(utc, millimeters)| {
                json!({
                    "datetime": "ignored",
                    "utcdatetime": utc,
                    "precipitation": millimeters,
                    "value": 0,
                })
            })
            .collect();
        let nowcast: RadarNowcast = serde_json::from_value(json!({
            "lastRefreshUtc": "2026-09-27T10:58:00",
            "forecasts": forecasts,
        }))
        .unwrap();
        nowcast.forecasts.into_iter().map(Downpour::from).collect()
    }

    #[test]
    fn radar_slots_decode_in_utc_and_millimeters_per_hour() {
        let forecast = decoded_forecast(&[("2026-09-27T23:55:00", 0.4)]);
        assert_eq!(forecast[0].at, september_27_utc(23, 55));
        assert_eq!(
            forecast[0].intensity,
            Velocity::new::<precipitation::millimeter_per_hour>(0.4)
        );
    }

    #[test]
    fn rain_reads_now_from_the_first_slot_and_peak_from_all() {
        let rain = Rain::from_forecast(decoded_forecast(&[
            ("2026-09-27T11:00:00", 0.2),
            ("2026-09-27T11:05:00", 3.0),
            ("2026-09-27T11:10:00", 1.0),
        ]))
        .unwrap();
        assert_eq!(rain.intensity, rain.forecast[0].intensity);
        assert_eq!(rain.peak.at, september_27_utc(11, 5));
    }

    #[test]
    fn an_empty_forecast_has_no_rain_reading() {
        assert!(Rain::from_forecast(Vec::new()).is_none());
    }

    #[test]
    fn dry_spell_starts_at_the_first_rain() {
        let forecast = decoded_forecast(&[
            ("2026-09-27T11:00:00", 0.0),
            ("2026-09-27T11:05:00", 0.0),
            ("2026-09-27T11:10:00", 1.2),
            ("2026-09-27T11:15:00", 0.0),
        ]);
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::StartsAt(september_27_utc(11, 10))
        );
    }

    #[test]
    fn ongoing_rain_stops_at_the_first_dry_moment() {
        let forecast = decoded_forecast(&[
            ("2026-09-27T23:50:00", 1.2),
            ("2026-09-27T23:55:00", 1.2),
            ("2026-09-28T00:00:00", 0.0),
        ]);
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::StopsAt(september_27_utc(23, 50) + TimeDelta::minutes(10))
        );
    }

    #[test]
    fn rain_through_the_whole_forecast_outlasts_it() {
        let forecast =
            decoded_forecast(&[("2026-09-27T11:00:00", 1.2), ("2026-09-27T11:05:00", 1.2)]);
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::OutlastsForecastUntil(september_27_utc(11, 5))
        );
    }

    #[test]
    fn no_rain_in_the_forecast_is_dry() {
        let forecast =
            decoded_forecast(&[("2026-09-27T11:00:00", 0.0), ("2026-09-27T11:05:00", 0.0)]);
        assert_eq!(RainOutlook::of(&forecast), RainOutlook::Dry);
    }
}
