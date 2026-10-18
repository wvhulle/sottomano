use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    data::source::gpu::{GpuProcesses, VramUsage},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
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
    fn name(&self) -> &'static str {
        "VRAM"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Memory(self.state().weight()).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fill())
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }
}

impl VramUsage {
    #[must_use]
    pub fn sections(&self) -> Vec<Section> {
        vec![Section::dictionary([
            KeyValue::new("VRAM", percent_text(self.fill())),
            KeyValue::new("Used", self.used.display().iec().to_string()),
            KeyValue::new("Total", self.total.display().iec().to_string()),
        ])]
    }
}

impl GpuProcesses {
    #[must_use]
    pub fn sections(&self) -> Vec<Section> {
        vec![
            Section::dictionary(self.processes.iter().filter_map(|process| {
                Some(KeyValue::new(
                    process
                        .name
                        .clone()
                        .unwrap_or_else(|| process.pid.to_string()),
                    process.vram?.display().iec().to_string(),
                ))
            }))
            .titled("Processes"),
        ]
    }
}
