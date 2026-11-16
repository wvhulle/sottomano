use chrono::Locale;
mod compression_setting_row;
mod ext4_error_count_row;
mod ext4_lifetime_writes_row;
mod filesystem_property_rows;
mod filesystem_usage_gauge;
mod inode_usage_row;
mod metadata_room_row;
mod mount_access_row;
mod subvolume_usage_row;
mod volume_identity_row;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    disk_storage::{
        bcachefs_filesystem_space::SnapshotDeletion,
        bcachefs_subvolume_usage::BcachefsSubvolumeUsage,
        block_filesystem_mounts::{BlockFilesystemDiscovery, FilesystemMount},
        mounted_filesystem_usage::{Filesystem, FilesystemUsage},
    },
};

pub use self::filesystem_property_rows::FilesystemProperty;
use self::{
    filesystem_usage_gauge::{
        SnapshotDeletionRow, filesystem_usage_hover_card, filesystem_usage_indicator,
    },
    subvolume_usage_row::{SubvolumeUsageRow, subvolume_usage_hover_card},
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const BCACHEFS_FS_TYPE: &str = "bcachefs";
const FILESYSTEM_USAGE_GLYPH: Phosphor = Phosphor::Folders(Weight::Regular);
const SUBVOLUME_USAGE_GLYPH: Phosphor = Phosphor::FolderSimple(Weight::Regular);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageSource {
    MountDiscovery,
    Usage(PathBuf),
    SubvolumeUsage(PathBuf),
}

pub(crate) fn block_filesystem_discovery_indicator(
    discovery: &BlockFilesystemDiscovery,
) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        BlockFilesystemDiscovery::NAME,
        Phosphor::Folders(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_panel_visibility(PanelVisibility::Hidden)
    .with_hover_card(Box::new(move |_| {
        block_filesystem_discovery_hover_card(discovery)
    }))
}

pub(crate) fn block_filesystem_discovery_hover_card(
    discovery: &BlockFilesystemDiscovery,
) -> Vec<Section> {
    vec![Section::new(
        "Mount points",
        dictionary_table(discovery.filesystems.iter().map(|filesystem| {
            LabeledValue::new(
                filesystem.primary_mount_point().display().to_string(),
                filesystem.fs_type.clone(),
            )
        })),
    )]
}

fn mount_group_title(primary_mount_point: &Path) -> WidgetGroupTitle {
    WidgetGroupTitle::Named(format!("Filesystem {}", primary_mount_point.display()).into())
}

struct MountedFilesystem {
    mount: FilesystemMount,
    usage: SubscribedSnapshot<FilesystemUsage>,
    subvolume_usage: SubscribedSnapshot<BcachefsSubvolumeUsage>,
    expansion: WidgetGroupExpansion,
}

#[derive(Default)]
pub struct StorageSpaceGroup {
    discovery: SubscribedSnapshot<BlockFilesystemDiscovery>,
    filesystems: BTreeMap<PathBuf, MountedFilesystem>,
    discovery_expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum StorageMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    Usage(PathBuf, Box<SnapshotDelivery<FilesystemUsage>>),
    SubvolumeUsage(PathBuf, Box<SnapshotDelivery<BcachefsSubvolumeUsage>>),
}

#[derive(Debug, Clone)]
pub enum StorageInteraction {
    DiscoveryExpansion(WidgetGroupExpansionMessage),
    MountExpansion {
        mount_point: PathBuf,
        expansion: WidgetGroupExpansionMessage,
    },
    Refresh(StorageSource),
}

impl StorageSpaceGroup {
    pub fn collapse(&mut self) {
        self.discovery_expansion.collapse();
        self.filesystems.values_mut().for_each(|filesystem| {
            filesystem.expansion.collapse();
        });
    }

    pub fn subscription(&self) -> Subscription<StorageMessage> {
        let usages = self.filesystems.iter().map(|(mount_point, filesystem)| {
            snapshot_subscription::<FilesystemUsage>(filesystem.mount.clone())
                .with(mount_point.clone())
                .map(|(mount_point, sample)| StorageMessage::Usage(mount_point, Box::new(sample)))
        });
        let subvolume_usages = self
            .filesystems
            .iter()
            .filter(|(_, filesystem)| filesystem.mount.fs_type == BCACHEFS_FS_TYPE)
            .map(|(mount_point, filesystem)| {
                snapshot_subscription::<BcachefsSubvolumeUsage>(filesystem.mount.clone())
                    .with(mount_point.clone())
                    .map(|(mount_point, sample)| {
                        StorageMessage::SubvolumeUsage(mount_point, Box::new(sample))
                    })
            });
        Subscription::batch(
            [
                snapshot_subscription::<BlockFilesystemDiscovery>(())
                    .map(StorageMessage::Discovered),
            ]
            .into_iter()
            .chain(usages)
            .chain(subvolume_usages),
        )
    }

    fn refresh(&self, source: StorageSource) -> Task<StorageMessage> {
        let discover =
            || first_delivery::<BlockFilesystemDiscovery>(()).map(StorageMessage::Discovered);
        match source {
            StorageSource::MountDiscovery => discover(),
            StorageSource::Usage(primary_mount_point) => {
                let Some(filesystem) = self.filesystems.get(&primary_mount_point) else {
                    return discover();
                };
                first_delivery::<FilesystemUsage>(filesystem.mount.clone()).map(move |sample| {
                    StorageMessage::Usage(primary_mount_point.clone(), Box::new(sample))
                })
            }
            StorageSource::SubvolumeUsage(primary_mount_point) => {
                let Some(filesystem) = self.filesystems.get(&primary_mount_point) else {
                    return discover();
                };
                first_delivery::<BcachefsSubvolumeUsage>(filesystem.mount.clone()).map(
                    move |sample| {
                        StorageMessage::SubvolumeUsage(
                            primary_mount_point.clone(),
                            Box::new(sample),
                        )
                    },
                )
            }
        }
    }

    pub fn update(&mut self, message: StorageMessage, clock: Clock) -> Task<StorageMessage> {
        match message {
            StorageMessage::Discovered(sample) => {
                self.discovery.receive(sample, clock);
                if let Some(discovery) = self.discovery.get() {
                    self.filesystems = discovery
                        .filesystems
                        .iter()
                        .map(|mount| {
                            let mount_point = mount.primary_mount_point().to_path_buf();
                            let filesystem = match self.filesystems.remove(&mount_point) {
                                Some(known) if &known.mount == mount => known,
                                known => MountedFilesystem {
                                    mount: mount.clone(),
                                    usage: SubscribedSnapshot::default(),
                                    subvolume_usage: SubscribedSnapshot::default(),
                                    expansion: known
                                        .map(|known| known.expansion)
                                        .unwrap_or_default(),
                                },
                            };
                            (mount_point, filesystem)
                        })
                        .collect();
                }
            }
            StorageMessage::Usage(mount_point, sample) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.usage.receive(*sample, clock);
                }
            }
            StorageMessage::SubvolumeUsage(mount_point, sample) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.subvolume_usage.receive(*sample, clock);
                }
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: StorageInteraction) -> Task<StorageMessage> {
        match interaction {
            StorageInteraction::DiscoveryExpansion(expansion) => {
                self.discovery_expansion.update(expansion);
            }
            StorageInteraction::MountExpansion {
                mount_point,
                expansion,
            } => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.expansion.update(expansion);
                }
            }
            StorageInteraction::Refresh(source) => {
                return self.refresh(source);
            }
        }
        Task::none()
    }

    fn property_items(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> impl Iterator<Item = GroupIndicator<'_>> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        filesystem.usage.get().into_iter().flat_map(move |usage| {
            FilesystemProperty::listed_for(usage)
                .into_iter()
                .filter_map(move |property| {
                    let row = property.row(usage)?;
                    let card_property = property.clone();
                    Some(
                        row.group_indicator(locale)
                            .with_hover_card(Box::new(move |_| {
                                card_property.row(usage).map_or_else(Vec::new, |card_row| {
                                    filesystem.usage.hover_card_with(card_row.hover_card())
                                })
                            }))
                            .refreshed_by(StorageInteraction::Refresh(StorageSource::Usage(
                                primary_mount_point.to_path_buf(),
                            ))),
                    )
                })
        })
    }

    fn subvolume_items(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> impl Iterator<Item = GroupIndicator<'_>> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        filesystem
            .usage
            .get()
            .zip(filesystem.subvolume_usage.get())
            .into_iter()
            .flat_map(move |(usage, subvolume_usage)| {
                subvolume_usage.subvolumes.iter().map(move |subvolume| {
                    SubvolumeUsageRow {
                        subvolume,
                        pool_total: usage.total,
                    }
                    .indicator(locale)
                    .with_hover_card(Box::new(move |_| {
                        filesystem.subvolume_usage.hover_card_with(
                            SubvolumeUsageRow {
                                subvolume,
                                pool_total: usage.total,
                            }
                            .hover_card(),
                        )
                    }))
                    .refreshed_by(StorageInteraction::Refresh(StorageSource::SubvolumeUsage(
                        primary_mount_point.to_path_buf(),
                    )))
                })
            })
    }

    fn mount_group(
        filesystem: &MountedFilesystem,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        let group_summary = match &filesystem.usage {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                Phosphor::Archive(Weight::Regular).into(),
                Self::filesystem_usage_item(filesystem, filesystem_usage_indicator(held_snapshot)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    Phosphor::Archive(Weight::Regular).into(),
                    Self::filesystem_usage_item(
                        filesystem,
                        troubled_snapshot.shown_over(
                            filesystem_usage_indicator(&troubled_snapshot.held_snapshot),
                            clock,
                        ),
                    ),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                Phosphor::Archive(Weight::Regular).into(),
                Self::filesystem_usage_item(
                    filesystem,
                    placeholder.indicator(FILESYSTEM_USAGE_GLYPH.into()),
                ),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupSummary::glyph(
                Phosphor::Archive(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupSummary::glyph(
                Phosphor::Archive(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        };
        let mut rows = Vec::new();
        rows.extend(Self::property_items(filesystem, locale));
        rows.extend(Self::subvolume_items(filesystem, locale));
        if let Some(failure) = filesystem.subvolume_usage.failure() {
            rows.push(Self::subvolume_failure_item(
                filesystem,
                failure.indicator(SUBVOLUME_USAGE_GLYPH.into()),
            ));
        }
        if let Some(usage) = filesystem.usage.get()
            && let Filesystem::Bcachefs(space) = &usage.filesystem
            && space.snapshot_deletion == SnapshotDeletion::Running
        {
            rows.push(Self::snapshot_deletion_item(filesystem));
        }
        let owned_mount_point = primary_mount_point.to_path_buf();
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(
                WidgetGroupFamily::StorageSpace,
                primary_mount_point.display().to_string(),
            ),
            mount_group_title(primary_mount_point),
            group_summary,
            &filesystem.expansion,
            move |expansion| StorageInteraction::MountExpansion {
                mount_point: owned_mount_point.clone(),
                expansion,
            },
            rows,
        )
    }

    fn filesystem_usage_item<'a>(
        filesystem: &'a MountedFilesystem,
        usage_row: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        usage_row
            .with_hover_card(Box::new(move |_| {
                filesystem
                    .usage
                    .sections_described_by(filesystem_usage_hover_card)
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::Usage(
                filesystem.mount.primary_mount_point().to_path_buf(),
            )))
    }

    fn subvolume_failure_item<'a>(
        filesystem: &'a MountedFilesystem,
        failure_row: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        failure_row
            .with_hover_card(Box::new(move |_| {
                filesystem
                    .subvolume_usage
                    .sections_described_by(subvolume_usage_hover_card)
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::SubvolumeUsage(
                filesystem.mount.primary_mount_point().to_path_buf(),
            )))
    }

    fn snapshot_deletion_item(filesystem: &MountedFilesystem) -> GroupIndicator<'_> {
        SnapshotDeletionRow::indicator()
            .with_hover_card(Box::new(move |_| {
                filesystem
                    .usage
                    .hover_card_with(SnapshotDeletionRow::hover_card())
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::Usage(
                filesystem.mount.primary_mount_point().to_path_buf(),
            )))
    }

    fn discovery_item<'a>(&'a self, discovery_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        discovery_row
            .with_hover_card(Box::new(move |_| {
                self.discovery
                    .sections_described_by(block_filesystem_discovery_hover_card)
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::MountDiscovery))
    }

    #[must_use]
    pub fn group_contents(&self, clock: Clock, locale: Locale) -> Vec<WidgetGroupContent<'_>> {
        if !self.filesystems.is_empty() {
            return self
                .filesystems
                .values()
                .map(|filesystem| Self::mount_group(filesystem, clock, locale))
                .collect();
        }
        let group_summary = match &self.discovery {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                Phosphor::Archive(Weight::Regular).into(),
                self.discovery_item(block_filesystem_discovery_indicator(held_snapshot)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    Phosphor::Archive(Weight::Regular).into(),
                    self.discovery_item(troubled_snapshot.shown_over(
                        block_filesystem_discovery_indicator(&troubled_snapshot.held_snapshot),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                Phosphor::Archive(Weight::Regular).into(),
                self.discovery_item(placeholder.indicator(FILESYSTEM_USAGE_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupSummary::glyph(
                Phosphor::Archive(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupSummary::glyph(
                Phosphor::Archive(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        };
        vec![WidgetGroupContent::expandable(
            WidgetGroupFamily::StorageSpace,
            group_summary,
            &self.discovery_expansion,
            StorageInteraction::DiscoveryExpansion,
            [],
        )]
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::mount_group_title;

    #[test]
    fn each_filesystem_gets_its_own_row_titled_by_its_mount_point() {
        assert_eq!(
            mount_group_title(Path::new("/home")).to_string(),
            "Filesystem /home"
        );
        assert_ne!(
            mount_group_title(Path::new("/")),
            mount_group_title(Path::new("/home"))
        );
    }
}
