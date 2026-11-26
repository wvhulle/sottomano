use chrono::Locale;
use sottomano_desktop_sources::disk_storage::{
    bcachefs_filesystem_space::{AllowedDeviceData, BcachefsDeviceSpace},
    block_filesystem_mounts::KernelDevice,
};
use sottomano_toml_config::severity::Severity;

use super::{DriveInteraction, DriveSource, PhysicalDiskSamples, PhysicalDriveGroup};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

const WRITABLE_MEMBER_STATE: &str = "rw";

pub(super) struct BcachefsMemberIndicator<'view> {
    pub device: &'view BcachefsDeviceSpace,
}

impl BcachefsMemberIndicator<'_> {
    fn writable_severity(&self) -> Severity {
        if self.device.state == WRITABLE_MEMBER_STATE {
            Severity::Normal
        } else {
            Severity::Warning
        }
    }

    fn used_percent(&self) -> i64 {
        let capacity = self.device.capacity.as_u64();
        if capacity == 0 {
            return 0;
        }
        let used = capacity
            .checked_sub(self.device.free.as_u64())
            .expect("the source rejects members with more reusable than total buckets");
        (used as f64 * 100.0 / capacity as f64).round() as i64
    }
}

impl<'view> BcachefsMemberIndicator<'view> {
    pub(super) fn indicator(self, locale: Locale) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.device.partition.clone(),
            Phosphor::HardDrives(Weight::Regular).into(),
            self.writable_severity().into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(format!(
                "{} free",
                Quantity::size(self.device.free).localized_text(locale)
            )),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        let metadata_text = match self.device.allowed_data {
            AllowedDeviceData::DataAndMetadata => "data and metadata",
            AllowedDeviceData::DataOnly => "data only",
        };
        vec![Section::new(
            "Device",
            dictionary_table([
                LabeledValue::new("Device label", self.device.label.clone()),
                LabeledValue::new("Partition", self.device.partition.clone()),
                LabeledValue::new("State", self.device.state.clone())
                    .level(self.writable_severity()),
                LabeledValue::new("Used", Quantity::share(percent_fill(self.used_percent()))),
                LabeledValue::new("Free", Quantity::size(self.device.free)),
                LabeledValue::new("Capacity", Quantity::size(self.device.capacity)),
                LabeledValue::new("Holds", metadata_text),
            ]),
        )]
    }
}

impl PhysicalDriveGroup {
    pub(super) fn bcachefs_member_listing<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        locale: Locale,
    ) -> IndicatorListing<'view> {
        IndicatorListing::Listed(Self::member_indicators(disk, samples, locale).collect())
    }

    fn member_indicators<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        locale: Locale,
    ) -> impl Iterator<Item = GroupIndicator<'view>> {
        samples
            .space
            .get()
            .into_iter()
            .flat_map(|occupancy| &occupancy.filesystem_shares)
            .flat_map(|share| &share.bcachefs_members)
            .map(move |device| {
                BcachefsMemberIndicator { device }
                    .indicator(locale)
                    .with_hover_card(Box::new(move |_| {
                        samples
                            .space
                            .hover_card_with(BcachefsMemberIndicator { device }.hover_card())
                    }))
                    .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                        disk.name.clone(),
                    )))
            })
    }
}
