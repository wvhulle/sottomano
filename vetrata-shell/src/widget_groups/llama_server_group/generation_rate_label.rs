use uom::si::{f64::Frequency, frequency::hertz};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::{Phosphor, Weight},
    tooltip_body::{KeyValue, TooltipBody},
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
        Phosphor::Lightning(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.text())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("Decoding", self.text())])
    }
}
