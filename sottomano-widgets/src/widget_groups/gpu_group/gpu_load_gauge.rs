use sottomano_desktop_sources::{SnapshotSource, cpu_memory_compute::pci_gpu_samples::GpuSnapshot};

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
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

pub fn gpu_load_indicator(snapshot: &GpuSnapshot) -> ReadingRow<'_> {
    let glyph = snapshot
        .vendor
        .vendor_logo()
        .map_or(GPU_SNAPSHOT_FALLBACK_GLYPH, IconGlyph::from);
    ReadingRow {
        readout: IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: snapshot.load,
                live: snapshot.load,
            },
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            GpuSnapshot::NAME,
            glyph,
            IndicatorState::Idle,
            Box::new(move |_| gpu_load_hover_card(snapshot)),
        )
    }
}

pub fn gpu_load_hover_card(snapshot: &GpuSnapshot) -> Vec<Section> {
    vec![Section::new(
        "Load",
        dictionary_table([LabeledValue::new("GPU", Quantity::share(snapshot.load))]),
    )]
}
