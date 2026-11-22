use chrono::Locale;
use sottomano_desktop_sources::disk_storage::block_filesystem_mounts::KernelDevice;

use super::{
    DriveInteraction, DriveSource, HARD_DRIVES_GLYPH, PhysicalDiskSamples, PhysicalDriveGroup,
    bcachefs_member_indicator::BcachefsMemberIndicator,
    disk_group_title,
    disk_space_gauge::{DiskMarking, disk_space_hover_card, disk_space_indicator},
    disk_transfer_gauge::{
        disk_activity_indicator, disk_transfer_hover_card, disk_transfer_indicator,
    },
    drive_health_indicator::{drive_health_hover_card, drive_health_indicator},
    refresh_activity,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    indicator_groups::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
};

const DRIVE_HEALTH_GLYPH: Phosphor = Phosphor::Thermometer(Weight::Regular);
const DISK_ACTIVITY_GLYPH: Phosphor = Phosphor::TrayArrowUp(Weight::Regular);

impl PhysicalDriveGroup {
    fn disk_marking(&self, disk: &KernelDevice) -> DiskMarking {
        if self.disks.len() < 2 {
            return DiskMarking::OnlyDisk;
        }
        self.disks
            .keys()
            .position(|known| known == disk)
            .map(|position| u8::try_from(position + 1).expect("fewer than 255 disks are attached"))
            .map_or(DiskMarking::OnlyDisk, |ordinal| DiskMarking::Numbered {
                ordinal,
            })
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

    pub(super) fn disk_group<'view>(
        &'view self,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        locale: Locale,
    ) -> IndicatorGroupContent<'view> {
        let headline_indicator = self.disk_space_headline(disk, samples);
        let mut rows: Vec<_> = Self::member_indicators(disk, samples, locale).collect();
        Self::push_drive_health_indicators(&mut rows, disk, samples);
        Self::push_disk_activity_indicators(&mut rows, disk, samples);
        rows.extend(Self::transfer_indicators(disk, samples));
        self.list_disk_space(disk, samples, &mut rows);
        IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(IndicatorGroupFamily::PhysicalDrive, disk.name.clone()),
            disk_group_title(disk),
            headline_indicator,
            &samples.expansion,
            move |expansion| DriveInteraction::DiskExpansion {
                disk: disk.clone(),
                expansion,
            },
            rows,
        )
    }

    fn disk_space_headline<'view>(
        &self,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) -> GroupIndicator<'view> {
        Self::wired_disk_space_indicator(
            disk,
            samples,
            samples.space.indicator(
                |held_snapshot| disk_space_indicator(held_snapshot, self.disk_marking(disk)),
                HARD_DRIVES_GLYPH.into(),
            ),
        )
        .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into())
    }

    fn list_disk_space<'view>(
        &self,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        indicators: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !samples.space.awaits_first_delivery() {
            indicators.insert(
                0,
                Self::wired_disk_space_indicator(
                    disk,
                    samples,
                    samples.space.indicator(
                        |held_snapshot| {
                            disk_space_indicator(held_snapshot, self.disk_marking(disk))
                        },
                        HARD_DRIVES_GLYPH.into(),
                    ),
                ),
            );
        }
    }

    fn push_drive_health_indicators<'view>(
        rows: &mut Vec<GroupIndicator<'view>>,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) {
        if !samples.drive_health.awaits_first_delivery() {
            let health_indicator = samples.drive_health.indicator(
                |held_snapshot| drive_health_indicator(held_snapshot),
                DRIVE_HEALTH_GLYPH.into(),
            );
            if health_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(Self::wired_drive_health_indicator(
                    disk,
                    samples,
                    health_indicator,
                ));
            }
        }
    }

    fn push_disk_activity_indicators<'view>(
        rows: &mut Vec<GroupIndicator<'view>>,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) {
        if !samples.activity.awaits_first_delivery() {
            let activity_indicator = samples.activity.indicator(
                |held_snapshot| disk_activity_indicator(held_snapshot),
                DISK_ACTIVITY_GLYPH.into(),
            );
            if activity_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(Self::wired_disk_activity_indicator(
                    disk,
                    samples,
                    activity_indicator,
                ));
            }
        }
    }

    fn wired_disk_space_indicator<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        space_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        space_indicator
            .with_hover_card(Box::new(move |_| {
                samples.space.sections_described_by(disk_space_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                disk.name.clone(),
            )))
    }

    fn wired_drive_health_indicator<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        health_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        health_indicator
            .with_hover_card(Box::new(move |_| {
                samples
                    .drive_health
                    .sections_described_by(drive_health_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Health(
                disk.name.clone(),
            )))
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
