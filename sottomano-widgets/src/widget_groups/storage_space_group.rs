mod filesystem_usage_gauge;

use std::{cmp::Reverse, collections::BTreeMap, path::PathBuf};

use filesystem_usage_gauge::SpaceSeverity;
use iced::{Subscription, Task};
use sottomano_config::severity::Severity;
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
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        named_reading::NamedReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody, tooltip_source_delivery::Tooltip},
        value_cells::LabeledValue,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

const ROOT_MOUNT_POINT: &str = "/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageTarget {
    MountDiscovery,
    Usage(PathBuf),
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

impl BarWidget for BlockFilesystemDiscovery {
    fn name(&self) -> &'static str {
        "Mounted filesystems"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Database(Weight::Regular).into()
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

struct MountedFilesystem {
    mount: FilesystemMount,
    usage: LatestSnapshot<FilesystemUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SpacePressure {
    severity: Severity,
    used_pct: i64,
}

impl MountedFilesystem {
    fn space_pressure(&self) -> SpacePressure {
        self.usage.get().map_or(
            SpacePressure {
                severity: Severity::Normal,
                used_pct: 0,
            },
            |usage| SpacePressure {
                severity: usage.space_severity(),
                used_pct: usage.used_pct(),
            },
        )
    }
}

#[derive(Default)]
pub struct StorageSpaceGroup {
    discovery: LatestSnapshot<BlockFilesystemDiscovery>,
    filesystems: BTreeMap<PathBuf, MountedFilesystem>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum StorageMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    Usage(PathBuf, Box<SnapshotDelivery<FilesystemUsage>>),
    Expansion(ExpansionMessage),
}

impl StorageSpaceGroup {
    pub const NAME: &str = "Storage";

    pub fn collapse(&mut self) {
        self.expansion.collapse();
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
        let StorageTarget::Usage(mount_point) = target else {
            return discover();
        };
        let Some(filesystem) = self.filesystems.get(mount_point) else {
            return discover();
        };
        let mount_point = mount_point.clone();
        first_delivery::<FilesystemUsage>(filesystem.mount.clone())
            .map(move |reading| StorageMessage::Usage(mount_point.clone(), Box::new(reading)))
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
                            let usage = self
                                .filesystems
                                .remove(&mount_point)
                                .filter(|known| &known.mount == mount)
                                .map(|known| known.usage)
                                .unwrap_or_default();
                            let filesystem = MountedFilesystem {
                                mount: mount.clone(),
                                usage,
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
            StorageMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    fn filesystems_most_pressing_first(&self) -> Vec<&MountedFilesystem> {
        let mut ordered: Vec<&MountedFilesystem> = self.filesystems.values().collect();
        ordered.sort_by_key(|filesystem| Reverse(filesystem.space_pressure()));
        ordered
    }

    pub fn group_content(&self) -> GroupContent<'_, StorageMessage> {
        let names_mount_points = self.filesystems.len() > 1;
        let usage_items = self
            .filesystems_most_pressing_first()
            .into_iter()
            .filter_map(move |filesystem| {
                let mount_point = filesystem.mount.primary_mount_point();
                let target = StorageTarget::Usage(mount_point.to_path_buf());
                match filesystem.usage.get() {
                    Some(usage) if names_mount_points => Some(GroupItem::reading(
                        NamedReading {
                            reading: usage,
                            shown_name: mount_point.to_str().unwrap_or(ROOT_MOUNT_POINT),
                        },
                        target,
                    )),
                    _ => filesystem.usage.group_item(target),
                }
            });
        GroupContent::new(
            Self::NAME,
            &self.expansion,
            StorageMessage::Expansion,
            self.discovery
                .group_item(StorageTarget::MountDiscovery)
                .into_iter()
                .chain(usage_items),
        )
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &StorageTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            StorageTarget::MountDiscovery => self.discovery.tooltip(clock),
            StorageTarget::Usage(mount_point) => {
                self.filesystems.get(mount_point)?.usage.tooltip(clock)
            }
        }
    }
}
