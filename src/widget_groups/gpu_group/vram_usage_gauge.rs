use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::Ratio,
        ratio::{percent, ratio},
    },
};

use crate::{
    data::source::gpu::VramUsage,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
    },
};

impl VramUsage {
    #[allow(clippy::cast_precision_loss)]
    fn fill(&self) -> Ratio {
        Ratio::new::<ratio>(self.used.as_u64() as f64 / self.total.as_u64() as f64)
    }
}

impl BarWidget for VramUsage {
    fn icon(&self) -> Glyph {
        Phosphor::Memory(self.state().weight()).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fill())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[
            Section::untitled_dictionary([
                KeyValue::new(
                    "VRAM",
                    format!("{:.0}", self.fill().into_format_args(percent, Abbreviation)),
                ),
                KeyValue::new("Used", self.used.display().iec().to_string()),
                KeyValue::new("Total", self.total.display().iec().to_string()),
            ]),
            Section::dictionary(
                "Processes",
                self.processes.iter().filter_map(|process| {
                    Some(KeyValue::new(
                        process
                            .name
                            .clone()
                            .unwrap_or_else(|| process.pid.to_string()),
                        process.vram?.display().iec().to_string(),
                    ))
                }),
            ),
        ])
    }
}
