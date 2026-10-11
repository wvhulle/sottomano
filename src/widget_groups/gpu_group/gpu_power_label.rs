use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};

use crate::{
    data::source::gpu::GpuPowerDraw,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
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
        Phosphor::Lightning(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(watt_text(self.draw))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled_dictionary(
            [
                Some(KeyValue::new("Draw", watt_text(self.draw))),
                self.limit
                    .map(|limit| KeyValue::new("Limit", watt_text(limit))),
            ]
            .into_iter()
            .flatten(),
        )])
    }
}
