use sottomano_desktop_sources::{SnapshotSource, weather::open_meteo_wind::Wind};

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph, localized_quantities::Quantity, value_cells::LabeledValue,
        weather_icons::WeatherIcon,
    },
};

pub(crate) const WIND_FALLBACK_GLYPH: IconGlyph = IconGlyph::WeatherIcon(WeatherIcon::StrongWind);

pub fn wind_indicator(wind: &Wind) -> ReadingRow<'_> {
    let state = match wind.beaufort() {
        0..=6 => IndicatorState::Idle,
        7..=8 => IndicatorState::Warning,
        _ => IndicatorState::Critical,
    };
    ReadingRow {
        readout: IndicatorReadout {
            label: ReadoutLabel::Labeled(format!(
                "{} Bft {}",
                wind.beaufort(),
                wind.compass_point()
            )),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            Wind::NAME,
            WeatherIcon::StrongWind.into(),
            state,
            Box::new(move |_| wind_hover_card(wind)),
        )
    }
}

pub fn wind_hover_card(wind: &Wind) -> Vec<Section> {
    vec![Section::new(
        "Now",
        dictionary_table([
            LabeledValue::new("Speed", Quantity::wind_speed(wind.speed)),
            LabeledValue::new("Beaufort", Quantity::count(wind.beaufort())),
            LabeledValue::new("Gusts", Quantity::wind_speed(wind.gusts)),
            LabeledValue::new("From", wind.compass_point()),
            LabeledValue::new("Bearing", Quantity::angle(wind.direction)),
        ]),
    )]
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
            wind_indicator(&wind).readout.label,
            ReadoutLabel::Labeled("3 Bft SW".to_owned())
        );
    }
}
