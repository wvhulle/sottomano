use sottomano_sources::cpu_memory_compute::pci_gpu_readings::GpuSnapshot;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
        vendor_logo::VendorLogo,
    },
};

impl GroupItemReading for GpuSnapshot {
    fn name(&self) -> &'static str {
        "GPU load"
    }

    fn icon(&self) -> IconGlyph {
        self.vendor
            .vendor_logo()
            .map_or_else(Self::fallback_icon, IconGlyph::from)
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::GraphicsCard(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.load)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([LabeledValue::new("GPU", Quantity::share(self.load))])
    }
}
