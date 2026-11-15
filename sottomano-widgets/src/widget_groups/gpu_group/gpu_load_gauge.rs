use std::borrow::Cow;

use sottomano_desktop_sources::{
    SnapshotSource, cpu_memory_compute::pci_gpu_readings::GpuSnapshot,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
        vendor_logo::VendorLogo,
    },
};

pub(crate) const GPU_SNAPSHOT_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::GraphicsCard(Weight::Regular));

impl GroupItemReading for GpuSnapshot {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        self.vendor
            .vendor_logo()
            .map_or(GPU_SNAPSHOT_FALLBACK_GLYPH, IconGlyph::from)
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.load)
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Load",
            dictionary_table([LabeledValue::new("GPU", Quantity::share(self.load))]),
        )]
    }
}
