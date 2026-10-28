use chrono::Locale;
use uom::si::{f64::Frequency, frequency::hertz};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
    },
};

pub struct GenerationRate(pub Frequency);

impl GenerationRate {
    fn text(&self) -> String {
        format!("{:.0} tok/s", self.0.get::<hertz>())
    }
}

impl BarWidget for GenerationRate {
    fn name(&self) -> &'static str {
        "Generation rate"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.text())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([LabeledValue::new(
            "Decoding",
            Quantity::measured(self.0.get::<hertz>(), DisplayUnit::TokensPerSecond, 0),
        )])
    }
}
