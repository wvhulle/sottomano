use sottomano_desktop_sources::{SnapshotSource, cpu_memory_compute::pci_gpu_samples::GpuSnapshot};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue, vendor_logo::VendorLogo},
};

pub(crate) const GPU_SNAPSHOT_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::GraphicsCard(Weight::Regular));

pub fn gpu_load_indicator(snapshot: &GpuSnapshot) -> GroupIndicator<'_> {
    let glyph = snapshot
        .vendor
        .vendor_logo()
        .map_or(GPU_SNAPSHOT_FALLBACK_GLYPH, IconGlyph::from);
    GroupIndicator::glyph_only(GpuSnapshot::NAME, glyph, IndicatorState::Idle)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: snapshot.load,
                live: snapshot.load,
            },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| gpu_load_hover_card(snapshot)))
}

pub fn gpu_load_hover_card(snapshot: &GpuSnapshot) -> Vec<Section> {
    vec![Section::new(
        "Load",
        dictionary_table([LabeledValue::new("GPU", Quantity::share(snapshot.load))]),
    )]
}
