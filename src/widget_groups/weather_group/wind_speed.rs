use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{angle::degree, f64::Velocity, velocity::kilometer_per_hour},
};

use crate::{
    data::source::weather::Wind,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        tooltip_body::{TooltipBody, row},
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
    },
};

const BEAUFORT_LOWER_BOUNDS_KM_PER_H: [f64; 12] = [
    1.0, 6.0, 12.0, 20.0, 29.0, 39.0, 50.0, 62.0, 75.0, 89.0, 103.0, 118.0,
];

const COMPASS_POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

fn speed_text(speed: Velocity) -> String {
    format!(
        "{:.0}",
        speed.into_format_args(kilometer_per_hour, Abbreviation)
    )
}

impl Wind {
    fn beaufort(&self) -> usize {
        BEAUFORT_LOWER_BOUNDS_KM_PER_H
            .iter()
            .take_while(|&&bound| self.speed >= Velocity::new::<kilometer_per_hour>(bound))
            .count()
    }

    fn compass_point(&self) -> &'static str {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sector = (self.direction.get::<degree>().rem_euclid(360.0) / 45.0).round() as usize;
        COMPASS_POINTS[sector % COMPASS_POINTS.len()]
    }
}

impl BarWidget for Wind {
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
        TooltipBody::rows(vec![
            row("Speed", speed_text(self.speed)),
            row("Beaufort", self.beaufort().to_string()),
            row("Gusts", speed_text(self.gusts)),
            row("From", self.compass_point()),
            row(
                "Bearing",
                format!(
                    "{:.0}",
                    self.direction.into_format_args(degree, Abbreviation)
                ),
            ),
        ])
    }
}
