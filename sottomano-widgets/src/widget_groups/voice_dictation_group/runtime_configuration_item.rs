use std::time::Duration;

use notulist_client::DictationEngine;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::HoverCardBody,
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
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

impl GroupItemReading for RuntimeConfiguration<'_> {
    fn name(&self) -> &'static str {
        "Runtime configuration"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::SlidersHorizontal(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::SlidersHorizontal(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(self.configuration_rows())
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
