use sottomano_sources::cpu_memory_compute::pci_gpu_readings::GpuSnapshot;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody},
        vendor_logo::VendorLogo,
    },
};

impl BarWidget for GpuSnapshot {
    fn name(&self) -> &'static str {
        "GPU load"
    }

    fn icon(&self) -> Glyph {
        self.vendor
            .vendor_logo()
            .map_or_else(Self::fallback_icon, Glyph::from)
    }

    fn fallback_icon() -> Glyph {
        Phosphor::GraphicsCard(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.load)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("GPU", Quantity::share(self.load))])
    }
}
