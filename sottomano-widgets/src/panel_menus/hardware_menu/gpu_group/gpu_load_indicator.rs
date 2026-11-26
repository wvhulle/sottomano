use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::pci_gpu_samples::{GpuSnapshot, PciBusId},
};

use super::{
    GpuGroup, GpuInteraction,
    gpu_device_access::{GpuDevice, GpuDeviceAccess},
};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity, subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue, vendor_logo::VendorLogo,
    },
};

pub(super) const GPU_LOAD_GLYPH: Phosphor = Phosphor::GraphicsCard(Weight::Regular);

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

impl GpuGroup {
    pub(super) fn gpu_load_listing<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
    ) -> IndicatorListing<'view> {
        match &device.access {
            GpuDeviceAccess::Readable { snapshot, .. } => snapshot
                .indicator_listing(
                    |held_snapshot| vec![gpu_load_indicator(held_snapshot)],
                    GPU_LOAD_GLYPH.into(),
                )
                .map_indicators(|load_row| {
                    self.wired_gpu_load_indicator(bus, device, snapshot, load_row)
                }),
            GpuDeviceAccess::Unreadable { handle_failure } => IndicatorListing::Incomplete {
                listed_indicators: Vec::new(),
                unread_sources: vec![
                    handle_failure
                        .indicator(GPU_LOAD_GLYPH.into())
                        .with_hover_card(Box::new(move |_| {
                            [handle_failure.source_state()]
                                .into_iter()
                                .chain(self.model_sections(device))
                                .collect()
                        }))
                        .refreshed_by(GpuInteraction::Refresh(bus.clone())),
                ],
            },
        }
    }
    pub(super) fn wired_gpu_load_indicator<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
        snapshot: &'view SubscribedSnapshot<GpuSnapshot>,
        load_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        load_indicator
            .with_hover_card(Box::new(move |_| {
                snapshot
                    .sections_described_by(gpu_load_hover_card)
                    .into_iter()
                    .chain(self.model_sections(device))
                    .collect()
            }))
            .refreshed_by(GpuInteraction::Refresh(bus.clone()))
    }
}
