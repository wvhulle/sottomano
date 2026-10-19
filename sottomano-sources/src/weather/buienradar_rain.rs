use std::time::Duration;

use chrono::{DateTime, Local, NaiveTime, TimeDelta};
use futures::Stream;
use situo_client::Position;
use uom::si::f64::Velocity;

use super::Error;
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, polled_delivery_stream,
    system_protocol_clients::http_json_client,
};

const BUIENRADAR_TIME_FORMAT: &str = "%H:%M";

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Downpour {
    pub intensity: Velocity,
    pub at: DateTime<Local>,
}

impl Downpour {
    fn is_wet(&self) -> bool {
        self.intensity > Velocity::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RainOutlook {
    Dry,
    StartsIn(TimeDelta),
    StopsIn(TimeDelta),
    OutlastsForecast(TimeDelta),
}

impl RainOutlook {
    fn of(forecast: &[Downpour]) -> Self {
        let Some((now, later)) = forecast.split_first() else {
            return RainOutlook::Dry;
        };
        if now.is_wet() {
            Self::while_raining(now, later)
        } else {
            Self::while_dry(now, later)
        }
    }

    fn while_dry(now: &Downpour, later: &[Downpour]) -> Self {
        later
            .iter()
            .find(|downpour| downpour.is_wet())
            .map_or(RainOutlook::Dry, |first_wet| {
                RainOutlook::StartsIn(first_wet.at - now.at)
            })
    }

    fn while_raining(now: &Downpour, later: &[Downpour]) -> Self {
        later
            .iter()
            .find(|downpour| !downpour.is_wet())
            .map_or_else(
                || {
                    let horizon = later.last().unwrap_or(now);
                    RainOutlook::OutlastsForecast(horizon.at - now.at)
                },
                |first_dry| RainOutlook::StopsIn(first_dry.at - now.at),
            )
    }
}

fn nearest_occurrence(clock: NaiveTime, now: DateTime<Local>) -> Option<DateTime<Local>> {
    [-1, 0, 1]
        .into_iter()
        .filter_map(|days| now.date_naive().checked_add_signed(TimeDelta::days(days)))
        .filter_map(|date| date.and_time(clock).and_local_timezone(Local).earliest())
        .min_by_key(|candidate| (*candidate - now).abs())
}

fn buienradar_intensity(value: f64) -> Velocity {
    let millimeters_per_hour = if value > 0.0 {
        10f64.powf((value - 109.0) / 32.0)
    } else {
        0.0
    };
    Velocity::new::<precipitation::millimeter_per_hour>(millimeters_per_hour)
}

fn downpour(line: &str, now: DateTime<Local>) -> Option<Downpour> {
    let (intensity, clock) = line.split_once('|')?;
    let clock = NaiveTime::parse_from_str(clock.trim(), BUIENRADAR_TIME_FORMAT).ok()?;
    Some(Downpour {
        intensity: buienradar_intensity(intensity.trim().parse().ok()?),
        at: nearest_occurrence(clock, now)?,
    })
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
            "https://gpsgadget.buienradar.nl/data/raintext?lat={}&lon={}",
            here.latitude, here.longitude
        );
        let body = http_json_client::get_text(&url, 5)
            .await
            .map_err(|error| match error {
                http_json_client::Error::Status { status: 404, .. } => Error::OutsideRainRadar,
                other => Error::from(other),
            })?;
        let now = Local::now();
        let forecast: Vec<Downpour> = body
            .lines()
            .filter_map(|line| downpour(line, now))
            .collect();
        let peak = forecast
            .iter()
            .max_by(|earlier, later| earlier.intensity.value.total_cmp(&later.intensity.value))
            .cloned()
            .ok_or(Error::Malformed)?;
        Ok(Rain {
            intensity: forecast
                .first()
                .map_or_else(Velocity::default, |now| now.intensity),
            peak,
            outlook: RainOutlook::of(&forecast),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn september_27_at(hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 9, 27, hour, minute, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn rain_lines_decode_into_downpours() {
        let now = september_27_at(9, 5);
        assert_eq!(
            downpour("087|09:10", now).map(|rain| rain.at),
            Some(september_27_at(9, 10))
        );
        assert!(downpour("087|9h10", now).is_none());
    }

    #[test]
    fn clock_times_past_midnight_belong_to_the_next_day() {
        let now = september_27_at(23, 55);
        let after_midnight = downpour("000|00:05", now).unwrap().at;
        assert_eq!(after_midnight - now, TimeDelta::minutes(10));
    }

    fn rain_forecast(now: DateTime<Local>, lines: &[&str]) -> Vec<Downpour> {
        lines
            .iter()
            .filter_map(|line| downpour(line, now))
            .collect()
    }

    #[test]
    fn dry_spell_counts_down_to_the_first_rain() {
        let forecast = rain_forecast(
            september_27_at(11, 0),
            &["000|11:00", "000|11:05", "077|11:10", "000|11:15"],
        );
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::StartsIn(TimeDelta::minutes(10))
        );
    }

    #[test]
    fn ongoing_rain_counts_down_to_the_first_dry_moment() {
        let forecast = rain_forecast(
            september_27_at(23, 50),
            &["077|23:50", "077|23:55", "000|00:00"],
        );
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::StopsIn(TimeDelta::minutes(10))
        );
    }

    #[test]
    fn rain_through_the_whole_forecast_outlasts_it() {
        let forecast = rain_forecast(september_27_at(11, 0), &["077|11:00", "077|11:05"]);
        assert_eq!(
            RainOutlook::of(&forecast),
            RainOutlook::OutlastsForecast(TimeDelta::minutes(5))
        );
    }

    #[test]
    fn no_rain_in_the_forecast_is_dry() {
        let forecast = rain_forecast(september_27_at(11, 0), &["000|11:00", "000|11:05"]);
        assert_eq!(RainOutlook::of(&forecast), RainOutlook::Dry);
    }
}
