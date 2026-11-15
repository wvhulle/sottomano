use uom::si::{f64::Frequency, frequency::hertz};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct GenerationRate<'a> {
    pub rate: Frequency,
    pub server: &'a str,
}

impl<'a> GenerationRate<'a> {
    pub fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(format!("{:.0} tok/s", self.rate.get::<hertz>())),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                format!("{} generation rate", self.server),
                Phosphor::Lightning(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |clock| self.hover_card(clock)),
            )
        }
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Throughput",
            dictionary_table([LabeledValue::new(
                "Decoding",
                Quantity::measured(self.rate.get::<hertz>(), DisplayUnit::TokensPerSecond, 0),
            )]),
        )]
    }
}
