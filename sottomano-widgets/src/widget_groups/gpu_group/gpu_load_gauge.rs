use sottomano_desktop_sources::{
    SnapshotSource, cpu_memory_compute::pci_gpu_readings::GpuSnapshot,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
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

pub fn gpu_load_reading_row(snapshot: &GpuSnapshot) -> ReadingRow<'_> {
    let glyph = snapshot
        .vendor
        .vendor_logo()
        .map_or(GPU_SNAPSHOT_FALLBACK_GLYPH, IconGlyph::from);
    ReadingRow {
        readout: RowReadout {
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
            ReadingDisplayState::Idle,
            Box::new(move |clock| gpu_load_hover_card(snapshot, clock)),
        )
    }
}

pub fn gpu_load_hover_card(snapshot: &GpuSnapshot, _clock: Clock) -> Vec<Section> {
    vec![Section::new(
        "Load",
        dictionary_table([LabeledValue::new("GPU", Quantity::share(snapshot.load))]),
    )]
}
