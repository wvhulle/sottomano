use sottomano_sources::weather::open_meteo_wind::Wind;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{angle::degree, f64::Velocity, velocity::kilometer_per_hour},
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    tooltip_body::{KeyValue, TooltipBody},
    weather_icons::WeatherIcon,
    widget_state::WidgetState,
};

const COMPASS_POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

fn speed_text(speed: Velocity) -> String {
    format!(
        "{:.0}",
        speed.into_format_args(kilometer_per_hour, Abbreviation)
    )
}

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

    fn label(&self) -> Option<String> {
        Some(self.beaufort().to_string())
    }

    fn state(&self) -> WidgetState {
        match self.beaufort() {
            0..=6 => WidgetState::Idle,
            7..=8 => WidgetState::Warning,
            _ => WidgetState::Critical,
        }
    }

    fn tooltip(&self) -> TooltipBody {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sector = (self.direction.get::<degree>().rem_euclid(360.0) / 45.0).round() as usize;
        TooltipBody::dictionary([
            KeyValue::new("Speed", speed_text(self.speed)),
            KeyValue::new("Beaufort", self.beaufort().to_string()),
            KeyValue::new("Gusts", speed_text(self.gusts)),
            KeyValue::new(
                "From",
                COMPASS_POINTS[sector % COMPASS_POINTS.len()].to_owned(),
            ),
            KeyValue::new(
                "Bearing",
                format!(
                    "{:.0}",
                    self.direction.into_format_args(degree, Abbreviation)
                ),
            ),
        ])
    }
}
