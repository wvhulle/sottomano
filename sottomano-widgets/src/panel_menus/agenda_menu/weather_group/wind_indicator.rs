use sottomano_desktop_sources::{SnapshotSource, weather::open_meteo_wind::Wind};

use super::{WeatherGroup, WeatherSource, refresh_online};
use crate::{
    glyph_icons::{icon_glyph::IconGlyph, weather_icons::WeatherIcon},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub(crate) const WIND_FALLBACK_GLYPH: IconGlyph = IconGlyph::WeatherIcon(WeatherIcon::StrongWind);

pub fn wind_indicator(wind: &Wind) -> GroupIndicator<'_> {
    let state = match wind.beaufort() {
        0..=6 => IndicatorState::Idle,
        7..=8 => IndicatorState::Warning,
        _ => IndicatorState::Critical,
    };
    GroupIndicator::glyph_only(Wind::NAME, WeatherIcon::StrongWind.into(), state)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(format!(
                "{} Bft {}",
                wind.beaufort(),
                wind.compass_point()
            )),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| wind_hover_card(wind)))
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

impl WeatherGroup {
    pub(super) fn wind_listing(&self) -> IndicatorListing<'_> {
        self.wind
            .indicator_listing(
                |held_snapshot| match wind_indicator(held_snapshot) {
                    wind_row if wind_row.panel_visibility == PanelVisibility::Hidden => Vec::new(),
                    wind_row => vec![wind_row],
                },
                WIND_FALLBACK_GLYPH,
            )
            .map_indicators(|wind_row| self.wired_wind_indicator(wind_row))
    }

    fn wired_wind_indicator<'view>(
        &'view self,
        wind_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        wind_row
            .with_hover_card(Box::new(move |_| {
                self.wind.sections_described_by(wind_hover_card)
            }))
            .refreshed_by(refresh_online(WeatherSource::Wind))
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
            wind_indicator(&wind)
                .shown_readout()
                .expect("a readout indicator")
                .label,
            ReadoutLabel::Labeled("3 Bft SW".to_owned())
        );
    }
}
