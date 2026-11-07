use std::{
    cmp::Reverse,
    fmt::{Display, Formatter, Result as FmtResult},
};

use chrono::{DateTime, Local, TimeDelta, Utc};
use sottomano_desktop_sources::weather::{
    buienradar_rain::{Downpour, Rain},
    open_meteo_precipitation_model::{ModelQuarterHour, PrecipitationModel},
    open_meteo_temperature::SkyWetness,
};
use uom::si::f64::Velocity;

use crate::{
    clock::Clock,
    widget_groups::weather_group::precipitation_type_forecast::{
        MODEL_QUARTER_HOUR, quarters_not_yet_over,
    },
};

pub const RADAR_FRAME_LENGTH: TimeDelta = TimeDelta::minutes(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RainForecastMethod {
    NumericalWeatherModel,
    RadarNowcast,
}

impl Display for RainForecastMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(match self {
            RainForecastMethod::NumericalWeatherModel => "model",
            RainForecastMethod::RadarNowcast => "radar",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RainRateReading<'a> {
    RadarFrame(&'a Downpour),
    ModelQuarter(&'a ModelQuarterHour),
}

impl RainRateReading<'_> {
    pub fn starts_at(self) -> DateTime<Utc> {
        match self {
            RainRateReading::RadarFrame(frame) => frame.at.to_utc(),
            RainRateReading::ModelQuarter(quarter) => quarter.starts_at,
        }
    }

    fn ends_at(self) -> DateTime<Utc> {
        match self {
            RainRateReading::RadarFrame(frame) => frame.at.to_utc() + RADAR_FRAME_LENGTH,
            RainRateReading::ModelQuarter(quarter) => quarter.starts_at + MODEL_QUARTER_HOUR,
        }
    }

    pub fn intensity(self) -> Velocity {
        match self {
            RainRateReading::RadarFrame(frame) => frame.intensity,
            RainRateReading::ModelQuarter(quarter) => quarter.intensity,
        }
    }

    pub fn sky_wetness(self) -> SkyWetness {
        match self {
            RainRateReading::RadarFrame(frame) if frame.intensity > Velocity::default() => {
                SkyWetness::Precipitating
            }
            RainRateReading::RadarFrame(_) => SkyWetness::Dry,
            RainRateReading::ModelQuarter(quarter) => quarter.condition.sky_wetness(),
        }
    }
}

pub struct RankedRateSource<'a> {
    pub method: RainForecastMethod,
    pub provider_name: &'static str,
    pub readings: Vec<RainRateReading<'a>>,
}

pub trait RankedRainProvider {
    const METHOD: RainForecastMethod;
    const PROVIDER_NAME: &'static str;

    fn upcoming_rate_readings(&self, clock: Clock) -> Vec<RainRateReading<'_>>;

    fn ranked_rate_source(&self, clock: Clock) -> RankedRateSource<'_> {
        RankedRateSource {
            method: Self::METHOD,
            provider_name: Self::PROVIDER_NAME,
            readings: self.upcoming_rate_readings(clock),
        }
    }
}

pub fn frames_not_yet_over(forecast: &[Downpour], now: DateTime<Local>) -> &[Downpour] {
    let elapsed = forecast
        .iter()
        .take_while(|frame| frame.at + RADAR_FRAME_LENGTH <= now)
        .count();
    &forecast[elapsed..]
}

impl RankedRainProvider for Rain {
    const METHOD: RainForecastMethod = RainForecastMethod::RadarNowcast;
    const PROVIDER_NAME: &'static str = "Buienradar";

    fn upcoming_rate_readings(&self, clock: Clock) -> Vec<RainRateReading<'_>> {
        frames_not_yet_over(&self.forecast, clock.local())
            .iter()
            .map(RainRateReading::RadarFrame)
            .collect()
    }
}

impl RankedRainProvider for PrecipitationModel {
    const METHOD: RainForecastMethod = RainForecastMethod::NumericalWeatherModel;
    const PROVIDER_NAME: &'static str = "Open-Meteo";

    fn upcoming_rate_readings(&self, clock: Clock) -> Vec<RainRateReading<'_>> {
        quarters_not_yet_over(self, clock.utc())
            .map(RainRateReading::ModelQuarter)
            .collect()
    }
}

pub fn ranked_by_method(mut rate_sources: Vec<RankedRateSource<'_>>) -> Vec<RankedRateSource<'_>> {
    rate_sources.sort_by_key(|source| Reverse(source.method));
    rate_sources
}

pub fn rate_readings_by_rank(rate_sources: Vec<RankedRateSource<'_>>) -> Vec<RainRateReading<'_>> {
    ranked_by_method(rate_sources)
        .into_iter()
        .fold(Vec::new(), |mut timeline, source| {
            let covered_until = timeline.last().copied().map(RainRateReading::ends_at);
            timeline.extend(source.readings.into_iter().filter(|reading| {
                covered_until.is_none_or(|covered_until| reading.starts_at() >= covered_until)
            }));
            timeline
        })
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::weather::{
        buienradar_rain::precipitation::millimeter_per_hour,
        open_meteo_temperature::{Daylight, Intensity, WeatherCondition},
    };

    use super::*;

    fn radar_frame(at: DateTime<Local>) -> Downpour {
        Downpour {
            intensity: Velocity::new::<millimeter_per_hour>(0.4),
            at,
        }
    }

    fn model_quarter(starts_at: DateTime<Utc>) -> ModelQuarterHour {
        ModelQuarterHour {
            starts_at,
            intensity: Velocity::default(),
            condition: WeatherCondition::Drizzle(Intensity::Light),
            daylight: Daylight::Night,
        }
    }

    #[test]
    fn radar_outranks_the_model_and_the_model_continues_past_the_radar_horizon() {
        let now = Clock::real().local();
        let frames: Vec<_> = (0..4)
            .map(|position| radar_frame(now + RADAR_FRAME_LENGTH * position))
            .collect();
        let quarters: Vec<_> = (0..3)
            .map(|position| model_quarter(now.to_utc() + MODEL_QUARTER_HOUR * position))
            .collect();
        let model_source = RankedRateSource {
            method: RainForecastMethod::NumericalWeatherModel,
            provider_name: "model",
            readings: quarters.iter().map(RainRateReading::ModelQuarter).collect(),
        };
        let radar_source = RankedRateSource {
            method: RainForecastMethod::RadarNowcast,
            provider_name: "radar",
            readings: frames.iter().map(RainRateReading::RadarFrame).collect(),
        };
        let timeline = rate_readings_by_rank(vec![model_source, radar_source]);
        assert_eq!(
            timeline,
            [
                RainRateReading::RadarFrame(&frames[0]),
                RainRateReading::RadarFrame(&frames[1]),
                RainRateReading::RadarFrame(&frames[2]),
                RainRateReading::RadarFrame(&frames[3]),
                RainRateReading::ModelQuarter(&quarters[2]),
            ]
        );
    }

    #[test]
    fn an_empty_higher_ranked_source_leaves_the_whole_timeline_to_the_next() {
        let now = Clock::real().utc();
        let quarters = [model_quarter(now), model_quarter(now + MODEL_QUARTER_HOUR)];
        let timeline = rate_readings_by_rank(vec![
            RankedRateSource {
                method: RainForecastMethod::RadarNowcast,
                provider_name: "radar",
                readings: Vec::new(),
            },
            RankedRateSource {
                method: RainForecastMethod::NumericalWeatherModel,
                provider_name: "model",
                readings: quarters.iter().map(RainRateReading::ModelQuarter).collect(),
            },
        ]);
        assert_eq!(timeline.len(), 2);
    }
}
