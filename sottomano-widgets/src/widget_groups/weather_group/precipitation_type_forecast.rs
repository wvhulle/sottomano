use chrono::{DateTime, TimeDelta, Utc};
use sottomano_sources::weather::open_meteo_precipitation_model::{
    ModelQuarterHour, PrecipitationModel,
};

use crate::widgets::{icon_glyph::IconGlyph, weather_icons::WeatherIcon};

pub const MODEL_QUARTER_HOUR: TimeDelta = TimeDelta::minutes(15);

pub fn model_quarter_text(quarter: &ModelQuarterHour) -> String {
    if quarter.condition.is_precipitation() {
        quarter.condition.to_string()
    } else {
        "Dry".to_owned()
    }
}

pub fn model_quarter_glyph(quarter: &ModelQuarterHour, dry_glyph: IconGlyph) -> IconGlyph {
    if quarter.condition.is_precipitation() {
        WeatherIcon::Sky(quarter.condition, quarter.daylight).into()
    } else {
        dry_glyph
    }
}

pub fn quarters_not_yet_over(
    model: &PrecipitationModel,
    now: DateTime<Utc>,
) -> impl Iterator<Item = &ModelQuarterHour> {
    model
        .quarters
        .iter()
        .filter(move |quarter| quarter.starts_at + MODEL_QUARTER_HOUR > now)
}

#[cfg(test)]
mod tests {
    use sottomano_sources::weather::{
        buienradar_rain::precipitation::millimeter_per_hour,
        open_meteo_temperature::{Daylight, Intensity, WeatherCondition},
    };
    use uom::si::f64::Velocity;

    use super::*;
    use crate::clock::Clock;

    fn quarter(starts_at: DateTime<Utc>, condition: WeatherCondition) -> ModelQuarterHour {
        ModelQuarterHour {
            starts_at,
            intensity: Velocity::new::<millimeter_per_hour>(5.6),
            condition,
            daylight: Daylight::Day,
        }
    }

    #[test]
    fn wet_quarters_name_the_precipitation_type_and_dry_ones_read_dry() {
        let now = Clock::real().utc();
        assert_eq!(
            model_quarter_text(&quarter(now, WeatherCondition::Drizzle(Intensity::Light))),
            "Light drizzle"
        );
        assert_eq!(
            model_quarter_text(&quarter(now, WeatherCondition::MainlyClear)),
            "Dry"
        );
    }

    #[test]
    fn quarters_already_over_leave_the_schedule() {
        let now = Clock::real().utc();
        let model = PrecipitationModel {
            current: quarter(now, WeatherCondition::Fog),
            quarters: vec![
                quarter(now - MODEL_QUARTER_HOUR, WeatherCondition::Fog),
                quarter(now, WeatherCondition::Rain(Intensity::Moderate)),
            ],
        };
        let remaining: Vec<_> = quarters_not_yet_over(&model, now).collect();
        assert_eq!(remaining, [&model.quarters[1]]);
    }
}
