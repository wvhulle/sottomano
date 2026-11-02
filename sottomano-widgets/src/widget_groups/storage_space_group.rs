mod filesystem_usage_gauge;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_sources::{
    SnapshotDelivery,
    disk_storage::{
        self,
        block_filesystem_mounts::{BlockFilesystemDiscovery, FilesystemMount},
        mounted_filesystem_usage::FilesystemUsage,
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        latest_snapshot::LatestSnapshot,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody, tooltip_source_delivery::Tooltip},
        value_cells::LabeledValue,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageTarget {
    MountDiscovery,
    Usage(PathBuf),
    MountPoint {
        filesystem: PathBuf,
        mount_point: PathBuf,
    },
}

impl From<disk_storage::Error> for Section {
    fn from(error: disk_storage::Error) -> Self {
        Section::error(error)
    }
}

impl From<StorageTarget> for HoverTarget {
    fn from(target: StorageTarget) -> Self {
        HoverTarget::Storage(target)
    }
}

impl GroupItemReading for BlockFilesystemDiscovery {
    fn name(&self) -> &'static str {
        "Mounted filesystems"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Folders(Weight::Regular).into()
    }

    fn visible_in_bar(&self) -> bool {
        false
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(self.filesystems.iter().map(|filesystem| {
            LabeledValue::new(
                filesystem.primary_mount_point().display().to_string(),
                filesystem.fs_type.clone(),
            )
        }))
    }
}

struct MountPointLabel<'a> {
    mount: &'a FilesystemMount,
    usage: &'a FilesystemUsage,
    mount_point: &'a Path,
}

impl GroupItemReading for MountPointLabel<'_> {
    fn name(&self) -> &'static str {
        "Mount point"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::FolderSimple(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.mount_point.display().to_string())
    }

    fn state(&self, clock: Clock) -> WidgetState {
        self.usage.state(clock)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let sharing_mount_points = self
            .mount
            .mount_points
            .iter()
            .filter(|other| other.as_path() != self.mount_point)
            .map(|other| other.display().to_string())
            .collect::<Vec<_>>();
        let sharing_entry = (!sharing_mount_points.is_empty())
            .then(|| LabeledValue::new("Shares space with", sharing_mount_points.join("  ")));
        TooltipBody::dictionary(
            [
                LabeledValue::new("Mount point", self.mount_point.display().to_string()),
                LabeledValue::new("Filesystem", self.mount.fs_type.clone()),
                LabeledValue::new("Free", Quantity::size(self.usage.free)),
            ]
            .into_iter()
            .chain(sharing_entry),
        )
    }
}

struct MountedFilesystem {
    mount: FilesystemMount,
    usage: LatestSnapshot<FilesystemUsage>,
    expansion: GroupExpansion,
}

#[derive(Default)]
pub struct StorageSpaceGroup {
    discovery: LatestSnapshot<BlockFilesystemDiscovery>,
    filesystems: BTreeMap<PathBuf, MountedFilesystem>,
    discovery_expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum StorageMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    Usage(PathBuf, Box<SnapshotDelivery<FilesystemUsage>>),
    DiscoveryExpansion(ExpansionMessage),
    MountExpansion {
        mount_point: PathBuf,
        expansion: ExpansionMessage,
    },
}

impl WidgetGroup for StorageSpaceGroup {
    const TITLE: &'static str = "Filesystem";
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
                .map(|(mount_point, reading)| StorageMessage::Usage(mount_point, Box::new(reading)))
        });
        Subscription::batch(
            [
                snapshot_subscription::<BlockFilesystemDiscovery>(())
                    .map(StorageMessage::Discovered),
            ]
            .into_iter()
            .chain(usages),
        )
    }

    pub fn refresh(&self, target: &StorageTarget) -> Task<StorageMessage> {
        let discover =
            || first_delivery::<BlockFilesystemDiscovery>(()).map(StorageMessage::Discovered);
        let primary_mount_point = match target {
            StorageTarget::MountDiscovery => return discover(),
            StorageTarget::Usage(primary_mount_point)
            | StorageTarget::MountPoint {
                filesystem: primary_mount_point,
                ..
            } => primary_mount_point,
        };
        let Some(filesystem) = self.filesystems.get(primary_mount_point) else {
            return discover();
        };
        let primary_mount_point = primary_mount_point.clone();
        first_delivery::<FilesystemUsage>(filesystem.mount.clone()).map(move |reading| {
            StorageMessage::Usage(primary_mount_point.clone(), Box::new(reading))
        })
    }

    pub fn update(&mut self, msg: StorageMessage, clock: Clock) -> Task<StorageMessage> {
        match msg {
            StorageMessage::Discovered(reading) => {
                self.discovery.receive(reading, clock);
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
                                    usage: LatestSnapshot::default(),
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
            StorageMessage::Usage(mount_point, reading) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.usage.receive(*reading, clock);
                }
            }
            StorageMessage::DiscoveryExpansion(expansion) => {
                self.discovery_expansion.update(expansion);
            }
            StorageMessage::MountExpansion {
                mount_point,
                expansion,
            } => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.expansion.update(expansion);
                }
            }
        }
        Task::none()
    }

    fn mount_group(filesystem: &MountedFilesystem) -> GroupContent<'_, StorageMessage> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        let mount_point_items = filesystem.usage.get().into_iter().flat_map(move |usage| {
            filesystem
                .mount
                .mount_points
                .iter()
                .map(move |mount_point| {
                    GroupItem::reading(
                        MountPointLabel {
                            mount: &filesystem.mount,
                            usage,
                            mount_point,
                        },
                        StorageTarget::MountPoint {
                            filesystem: primary_mount_point.to_path_buf(),
                            mount_point: mount_point.clone(),
                        },
                    )
                })
        });
        let owned_mount_point = primary_mount_point.to_path_buf();
        Self::expandable_group(
            &filesystem.expansion,
            move |expansion| StorageMessage::MountExpansion {
                mount_point: owned_mount_point.clone(),
                expansion,
            },
            filesystem
                .usage
                .group_item(StorageTarget::Usage(primary_mount_point.to_path_buf()))
                .into_iter()
                .chain(mount_point_items),
        )
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<GroupContent<'_, StorageMessage>> {
        let discovery_group = Self::expandable_group(
            &self.discovery_expansion,
            StorageMessage::DiscoveryExpansion,
            self.discovery.group_item(StorageTarget::MountDiscovery),
        );
        [discovery_group]
            .into_iter()
            .chain(self.filesystems.values().map(Self::mount_group))
            .map(GroupContent::sharing_title_row)
            .collect()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &StorageTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            StorageTarget::MountDiscovery => self.discovery.tooltip(clock),
            StorageTarget::Usage(mount_point) => {
                self.filesystems.get(mount_point)?.usage.tooltip(clock)
            }
            StorageTarget::MountPoint {
                filesystem,
                mount_point,
            } => {
                let filesystem = self.filesystems.get(filesystem)?;
                let usage = filesystem.usage.get()?;
                let mount_point = filesystem
                    .mount
                    .mount_points
                    .iter()
                    .find(|known| *known == mount_point)?;
                filesystem.usage.tooltip_with(
                    MountPointLabel {
                        mount: &filesystem.mount,
                        usage,
                        mount_point,
                    }
                    .tooltip(clock),
                )
            }
        }
    }
}
