use chrono::Locale;
use sottomano_sources::cpu_memory_compute::pci_gpu_readings::GpuPowerDraw;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody},
    },
};

fn watt_text(power: Power) -> String {
    format!("{:.0}", power.into_format_args(watt, Abbreviation))
}

impl BarWidget for GpuPowerDraw {
    fn name(&self) -> &'static str {
        "GPU power"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Lightning(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(watt_text(self.draw))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(
            [
                Some(KeyValue::new(
                    "Draw",
                    Quantity::power(self.draw).decimals(0),
                )),
                self.limit
                    .map(|limit| KeyValue::new("Limit", Quantity::power(limit).decimals(0))),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
