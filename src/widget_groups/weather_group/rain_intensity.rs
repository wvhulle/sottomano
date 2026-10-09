use uom::{fmt::DisplayStyle::Abbreviation, si::f64::Velocity};

use crate::{
    data::{
        severity::Severity,
        source::weather::{Rain, RainOutlook, precipitation::millimeter_per_hour},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        localized_time::TimePart,
        tooltip_body::{Cell, Row, Section, TooltipBody},
        weather_icons::WeatherIcon,
    },
};

fn intensity_text(intensity: Velocity) -> String {
    format!(
        "{:.1}",
        intensity.into_format_args(millimeter_per_hour, Abbreviation)
    )
}

impl BarWidget for Rain {
    fn icon(&self) -> Glyph {
        let peak = self.peak.intensity;
        if peak < Velocity::new::<millimeter_per_hour>(0.5) {
            WeatherIcon::Raindrop.into()
        } else if peak < Velocity::new::<millimeter_per_hour>(7.5) {
            WeatherIcon::Rain.into()
        } else {
            WeatherIcon::Thunderstorm.into()
        }
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::Raindrop.into()
    }

    fn label(&self) -> Option<String> {
        match self.outlook {
            RainOutlook::Dry => None,
            RainOutlook::StartsIn(until_wet) => Some(format!("in {}m", until_wet.num_minutes())),
            RainOutlook::StopsIn(until_dry) => Some(format!("for {}m", until_dry.num_minutes())),
            RainOutlook::OutlastsForecast(horizon) => {
                Some(format!("for {}m+", horizon.num_minutes()))
            }
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let now = if self.intensity <= Velocity::default() {
            "none right now".to_string()
        } else {
            intensity_text(self.intensity)
        };
        TooltipBody::sections(&[Section::untitled(&[
            ("Rain", now).into(),
            Row::new(
                "Peak",
                &[
                    intensity_text(self.peak.intensity).into(),
                    Cell::Time {
                        at: self.peak.at.to_utc(),
                        part: TimePart::TimeOfDay,
                    },
                ],
                Severity::Normal,
            ),
        ])])
    }
}

impl Occasional for Rain {
    fn present(&self) -> bool {
        self.outlook != RainOutlook::Dry
    }
}

#[cfg(test)]
mod tests {
    use uom::si::{
        f64::{Length, Time},
        length::millimeter,
        time::hour,
    };

    use super::*;

    #[test]
    fn millimeter_per_hour_is_a_millimeter_each_hour() {
        let from_unit = Velocity::new::<millimeter_per_hour>(2.5);
        let from_parts = Length::new::<millimeter>(2.5) / Time::new::<hour>(1.0);
        assert!((from_unit - from_parts).abs() < Velocity::new::<millimeter_per_hour>(1e-9));
    }

    #[test]
    fn intensity_prints_in_millimeters_per_hour() {
        assert_eq!(
            intensity_text(Velocity::new::<millimeter_per_hour>(2.5)),
            "2.5 mm/h"
        );
    }
}
