use sottomano_desktop_sources::{
    SnapshotSource,
    disk_storage::{
        block_filesystem_mounts::KernelDevice,
        disk_io_activity::{DiskActivity, DiskTransfer, DiskTransferDirection},
    },
};
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{gibibyte_per_second, kibibyte_per_second},
    ratio::ratio,
};

use super::{PhysicalDiskSamples, PhysicalDriveGroup, refresh_activity};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::ReadoutGauge,
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

const BACKGROUND_TRANSFER_KIB_PER_S: f64 = 64.0;
const SCALE_END_GIB_PER_S: f64 = 4.0;

fn background_rate() -> InformationRate {
    InformationRate::new::<kibibyte_per_second>(BACKGROUND_TRANSFER_KIB_PER_S)
}

fn transfer_icon(transfer: &DiskTransfer) -> IconGlyph {
    let direction_icon = match transfer.direction {
        DiskTransferDirection::Read => Phosphor::TrayArrowUp(Weight::Regular),
        DiskTransferDirection::Write => Phosphor::TrayArrowDown(Weight::Regular),
    };
    direction_icon.with_weight(Weight::Regular).into()
}

fn transfer_fill(transfer: &DiskTransfer) -> Ratio {
    let scale_start = background_rate();
    let scale_end = InformationRate::new::<gibibyte_per_second>(SCALE_END_GIB_PER_S);
    if transfer.rate <= scale_start {
        return Ratio::new::<ratio>(0.0);
    }
    let scale_width = (scale_end / scale_start).get::<ratio>().ln();
    let position = (transfer.rate / scale_start).get::<ratio>().ln();
    Ratio::new::<ratio>((position / scale_width).clamp(0.0, 1.0))
}

pub(crate) fn disk_transfer_indicator(transfer: &DiskTransfer) -> GroupIndicator<'_> {
    let name = match transfer.direction {
        DiskTransferDirection::Read => "Disk reads",
        DiskTransferDirection::Write => "Disk writes",
    };
    let state = if transfer.rate <= background_rate() {
        IndicatorState::Off
    } else {
        IndicatorState::Idle
    };
    let fill = transfer_fill(transfer);
    GroupIndicator::glyph_only(name, transfer_icon(transfer), state)
        .with_gauge(ReadoutGauge::Gauged { fill, live: fill })
        .with_hover_card(Box::new(move |_| disk_transfer_hover_card(transfer)))
}

pub(crate) fn disk_activity_indicator(activity: &DiskActivity) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        DiskActivity::NAME,
        transfer_icon(&activity.read),
        IndicatorState::Idle,
    )
    .with_panel_visibility(PanelVisibility::Hidden)
    .with_hover_card(Box::new(move |_| disk_transfer_hover_card(&activity.read)))
}

pub(crate) fn disk_transfer_hover_card(transfer: &DiskTransfer) -> Vec<Section> {
    let rate_label = match transfer.direction {
        DiskTransferDirection::Read => "Read",
        DiskTransferDirection::Write => "Write",
    };
    vec![Section::new(
        "Throughput",
        dictionary_table([
            LabeledValue::new("Disk", transfer.disk.clone()),
            LabeledValue::new(rate_label, Quantity::transfer_rate(transfer.rate)),
            LabeledValue::new("Disk busy", Quantity::share(transfer.disk_busy)),
        ]),
    )]
}

const DISK_ACTIVITY_GLYPH: Phosphor = Phosphor::TrayArrowUp(Weight::Regular);

impl PhysicalDriveGroup {
    pub(super) fn disk_activity_listing<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) -> IndicatorListing<'view> {
        samples
            .activity
            .indicator_listing(
                |held_snapshot| disk_activity_indicator(held_snapshot).when_shown(),
                DISK_ACTIVITY_GLYPH.into(),
            )
            .map_indicators(|activity_row| {
                Self::wired_disk_activity_indicator(disk, samples, activity_row)
            })
    }

    pub(super) fn disk_transfer_listing<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) -> IndicatorListing<'view> {
        IndicatorListing::Listed(Self::transfer_indicators(disk, samples).collect())
    }

    fn transfer_indicators<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) -> impl Iterator<Item = GroupIndicator<'view>> {
        samples
            .activity
            .get()
            .into_iter()
            .flat_map(move |activity| {
                [
                    disk_transfer_indicator(&activity.read)
                        .with_hover_card(Box::new(move |_| {
                            samples
                                .activity
                                .hover_card_with(disk_transfer_hover_card(&activity.read))
                        }))
                        .refreshed_by(refresh_activity(disk)),
                    disk_transfer_indicator(&activity.write)
                        .with_hover_card(Box::new(move |_| {
                            samples
                                .activity
                                .hover_card_with(disk_transfer_hover_card(&activity.write))
                        }))
                        .refreshed_by(refresh_activity(disk)),
                ]
            })
    }

    fn wired_disk_activity_indicator<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        activity_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        activity_indicator
            .with_hover_card(Box::new(move |_| {
                samples
                    .activity
                    .sections_described_by(|snapshot| disk_transfer_hover_card(&snapshot.read))
            }))
            .refreshed_by(refresh_activity(disk))
    }
}
