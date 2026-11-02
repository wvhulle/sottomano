use chrono::Locale;
use uom::si::{f64::Frequency, frequency::hertz};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct GenerationRate(pub Frequency);

impl GenerationRate {
    fn text(&self) -> String {
        format!("{:.0} tok/s", self.0.get::<hertz>())
    }
}

impl GroupItemReading for GenerationRate {
    fn name(&self) -> &'static str {
        "Generation rate"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.text())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new(
            "Decoding",
            Quantity::measured(self.0.get::<hertz>(), DisplayUnit::TokensPerSecond, 0),
        )])
    }
}
