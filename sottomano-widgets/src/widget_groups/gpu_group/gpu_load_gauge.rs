use sottomano_sources::cpu_memory_compute::pci_gpu_readings::{GpuSnapshot, GpuVendor};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, TooltipBody},
    },
};

impl BarWidget for GpuSnapshot {
    fn name(&self) -> &'static str {
        "GPU load"
    }

    fn icon(&self) -> Glyph {
        match self.vendor {
            GpuVendor::Nvidia => SimpleIcon::NVIDIA.into(),
            GpuVendor::Intel => SimpleIcon::Intel.into(),
        }
    }

    fn fallback_icon() -> Glyph {
        Phosphor::GraphicsCard(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.load)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("GPU", percent_text(self.load))])
    }
}
