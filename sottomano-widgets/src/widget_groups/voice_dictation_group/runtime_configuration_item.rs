use std::time::Duration;

use chrono::Locale;
use notulist_client::DictationEngine;

use crate::{
    clock::Clock,
    group_items::{reading_display_state::ReadingDisplayState, reading_row::ReadingRow},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct RuntimeConfiguration<'a>(pub &'a DictationEngine);

impl RuntimeConfiguration<'_> {
    fn configuration_rows(&self) -> impl Iterator<Item = LabeledValue> {
        let engine = self.0;
        [
            Some(engine.inference_threads)
                .filter(|threads| *threads > 0)
                .map(Quantity::count)
                .map(|thread_count| LabeledValue::new("Threads", thread_count)),
            Some(LabeledValue::new(
                "Flash attention",
                if engine.flash_attention { "on" } else { "off" },
            )),
            Some(engine.retranscribe_interval_secs)
                .filter(|interval_secs| interval_secs.is_finite() && *interval_secs > 0.0)
                .map(|interval_secs| ValueCell::Duration(Duration::from_secs_f64(interval_secs)))
                .map(|retranscribe_interval| {
                    LabeledValue::new("Re-transcribe every", retranscribe_interval)
                }),
            Some(engine.sample_rate_hz)
                .filter(|sample_rate| *sample_rate > 0)
                .map(|sample_rate| {
                    Quantity::measured(f64::from(sample_rate), DisplayUnit::Hertz, 0)
                })
                .map(|sample_rate| LabeledValue::new("Sample rate", sample_rate)),
        ]
        .into_iter()
        .flatten()
    }
}

impl<'a> RuntimeConfiguration<'a> {
    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        ReadingRow::glyph_only(
            "Runtime configuration",
            Phosphor::SlidersHorizontal(Weight::Regular).into(),
            ReadingDisplayState::On,
            Box::new(move |card_clock| self.hover_card(card_clock)),
        )
    }

    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Settings",
            dictionary_table(self.configuration_rows()),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rows_leave_out_unset_threads_and_interval() {
        let engine = DictationEngine {
            flash_attention: true,
            sample_rate_hz: 16_000,
            ..DictationEngine::default()
        };
        let keys: Vec<ValueCell> = RuntimeConfiguration(&engine)
            .configuration_rows()
            .map(|row| row.key().clone())
            .collect();
        assert_eq!(
            keys,
            ["Flash attention", "Sample rate"]
                .map(ValueCell::from)
                .to_vec()
        );
    }

    #[test]
    fn configured_threads_and_interval_are_typed_cells() {
        let engine = DictationEngine {
            inference_threads: 8,
            retranscribe_interval_secs: 0.5,
            ..DictationEngine::default()
        };
        let values: Vec<ValueCell> = RuntimeConfiguration(&engine)
            .configuration_rows()
            .map(|row| row.value().clone())
            .collect();
        assert_eq!(
            values,
            [
                ValueCell::Quantity(Quantity::count(8_u32)),
                ValueCell::from("off"),
                ValueCell::Duration(Duration::from_millis(500)),
            ]
            .to_vec()
        );
    }
}
