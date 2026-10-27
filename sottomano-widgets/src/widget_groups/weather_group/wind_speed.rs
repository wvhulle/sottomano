use chrono::Locale;
use sottomano_sources::weather::open_meteo_wind::Wind;
use uom::si::angle::degree;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        tooltip_body::{KeyValue, TooltipBody},
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
    },
};

const COMPASS_POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

impl BarWidget for Wind {
    fn name(&self) -> &'static str {
        "Wind"
    }

    fn icon(&self) -> Glyph {
        WeatherIcon::StrongWind.into()
    }

    fn fallback_icon() -> Glyph {
        WeatherIcon::StrongWind.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.beaufort().to_string())
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.beaufort() {
            0..=6 => WidgetState::Idle,
            7..=8 => WidgetState::Warning,
            _ => WidgetState::Critical,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sector = (self.direction.get::<degree>().rem_euclid(360.0) / 45.0).round() as usize;
        TooltipBody::dictionary([
            KeyValue::new("Speed", Quantity::wind_speed(self.speed)),
            KeyValue::new("Beaufort", Quantity::count(self.beaufort())),
            KeyValue::new("Gusts", Quantity::wind_speed(self.gusts)),
            KeyValue::new(
                "From",
                COMPASS_POINTS[sector % COMPASS_POINTS.len()].to_owned(),
            ),
            KeyValue::new("Bearing", Quantity::angle(self.direction)),
        ])
    }
}
