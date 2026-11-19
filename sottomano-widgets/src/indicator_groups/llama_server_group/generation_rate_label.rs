use uom::si::{f64::Frequency, frequency::hertz};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        value_cells::LabeledValue,
    },
};

pub struct GenerationRate<'a> {
    pub rate: Frequency,
    pub server: &'a str,
}

impl<'a> GenerationRate<'a> {
    pub fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            format!("{} generation rate", self.server),
            Phosphor::Lightning(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(format!("{:.0} tok/s", self.rate.get::<hertz>())),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Throughput",
            dictionary_table([LabeledValue::new(
                "Decoding",
                Quantity::measured(self.rate.get::<hertz>(), DisplayUnit::TokensPerSecond, 0),
            )]),
        )]
    }
}
