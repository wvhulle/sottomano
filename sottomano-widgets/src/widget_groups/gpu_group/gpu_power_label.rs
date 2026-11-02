use chrono::Locale;
use sottomano_sources::cpu_memory_compute::pci_gpu_readings::GpuPowerDraw;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::HoverCardBody,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

fn watt_text(power: Power) -> String {
    format!("{:.0}", power.into_format_args(watt, Abbreviation))
}

impl GroupItemReading for GpuPowerDraw {
    fn name(&self) -> &'static str {
        "GPU power"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(watt_text(self.draw))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            [
                Some(LabeledValue::new(
                    "Draw",
                    Quantity::power(self.draw).decimals(0),
                )),
                self.limit
                    .map(|limit| LabeledValue::new("Limit", Quantity::power(limit).decimals(0))),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
