use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};
use vetrata_sources::gpu::GpuPowerDraw;

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    phosphor::Phosphor,
    tooltip_body::{KeyValue, TooltipBody},
};

fn watt_text(power: Power) -> String {
    format!("{:.0}", power.into_format_args(watt, Abbreviation))
}

impl BarWidget for GpuPowerDraw {
    fn name(&self) -> &'static str {
        "GPU power"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Lightning(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(watt_text(self.draw))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary(
            [
                Some(KeyValue::new("Draw", watt_text(self.draw))),
                self.limit
                    .map(|limit| KeyValue::new("Limit", watt_text(limit))),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
