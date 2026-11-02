use chrono::Locale;
use sottomano_sources::weather::open_meteo_wind::Wind;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph, localized_quantities::Quantity, value_cells::LabeledValue,
        weather_icons::WeatherIcon,
    },
};

pub(super) const WIND_ITEM_NAME: &str = "Wind";

impl GroupItemReading for Wind {
    fn name(&self) -> &'static str {
        WIND_ITEM_NAME
    }

    fn icon(&self) -> IconGlyph {
        WeatherIcon::StrongWind.into()
    }

    fn fallback_icon() -> IconGlyph {
        WeatherIcon::StrongWind.into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} Bft {}", self.beaufort(), self.compass_point()))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.beaufort() {
            0..=6 => ReadingDisplayState::Idle,
            7..=8 => ReadingDisplayState::Warning,
            _ => ReadingDisplayState::Critical,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Speed", Quantity::wind_speed(self.speed)),
            LabeledValue::new("Beaufort", Quantity::count(self.beaufort())),
            LabeledValue::new("Gusts", Quantity::wind_speed(self.gusts)),
            LabeledValue::new("From", self.compass_point()),
            LabeledValue::new("Bearing", Quantity::angle(self.direction)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use uom::si::{
        angle::degree,
        f64::{Angle, Velocity},
        velocity::kilometer_per_hour,
    };

    use super::*;

    #[test]
    fn the_panel_label_names_beaufort_and_compass_point() {
        let wind = Wind {
            speed: Velocity::new::<kilometer_per_hour>(15.0),
            gusts: Velocity::new::<kilometer_per_hour>(30.0),
            direction: Angle::new::<degree>(225.0),
        };
        assert_eq!(
            wind.label(Clock::real(), Locale::default()),
            Some("3 Bft SW".to_owned())
        );
    }
}
