use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsStr,
    fs,
    iter::successors,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::Stream;
use procfs::process::{MountInfo, Process};

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking_poll,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::sysfs_attribute_reads::{FileReadError, list_dir},
};

pub mod mount_compression_options;

use mount_compression_options::Compression;

const MOUNTINFO: &str = "/proc/self/mountinfo";
const SYS_CLASS_BLOCK: &str = "/sys/class/block";
const MULTI_DEVICE_SEPARATOR: char = ':';
const VIRTUAL_BLOCK_PREFIXES: [&str; 3] = ["loop", "ram", "zram"];
const ROOT_MOUNT_POINT: &str = "/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FilesystemMount {
    pub fs_type: String,
    pub mount_points: Vec<PathBuf>,
    pub devices: Vec<KernelDevice>,
    pub compression: Compression,
}

impl FilesystemMount {
    #[must_use]
    pub fn primary_mount_point(&self) -> &Path {
        self.mount_points
            .first()
            .map_or(Path::new(ROOT_MOUNT_POINT), PathBuf::as_path)
    }

    #[must_use]
    pub fn mount_role(&self) -> MountRole {
        if self
            .mount_points
            .iter()
            .any(|mount_point| mount_point == Path::new(ROOT_MOUNT_POINT))
        {
            MountRole::Root
        } else {
            MountRole::Secondary
        }
    }

    #[must_use]
    pub fn whole_disks(&self) -> Vec<KernelDevice> {
        let mut disks: Vec<KernelDevice> =
            self.devices.iter().map(KernelDevice::whole_disk).collect();
        disks.sort();
        disks.dedup();
        disks
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFilesystemDiscovery {
    pub filesystems: Vec<FilesystemMount>,
}

impl SnapshotSource for BlockFilesystemDiscovery {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Mounted filesystems";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(60))
            .poll_deliveries(|| blocking_poll(Self::read_mountinfo))
    }
}

impl BlockFilesystemDiscovery {
    pub fn read_mountinfo() -> Result<Self, FileReadError> {
        let mounts = Process::myself()
            .and_then(|process| process.mountinfo())
            .map_err(|_| FileReadError::Unreadable(MOUNTINFO.to_owned()))?;
        Ok(BlockFilesystemDiscovery {
            filesystems: group_block_mounts(
                mounts.into_iter().filter_map(MountEntry::sourced),
                KernelDevice::backing,
            ),
        })
    }

    #[must_use]
    pub fn whole_disks(&self) -> Vec<KernelDevice> {
        let mut disks: Vec<KernelDevice> = self
            .filesystems
            .iter()
            .flat_map(FilesystemMount::whole_disks)
            .collect();
        disks.sort();
        disks.dedup();
        disks
    }

    #[must_use]
    pub fn root_disks(&self) -> Vec<KernelDevice> {
        self.filesystems
            .iter()
            .filter(|filesystem| filesystem.mount_role() == MountRole::Root)
            .flat_map(FilesystemMount::whole_disks)
            .collect()
    }
}

struct MountEntry {
    mount_point: PathBuf,
    fs_type: String,
    source: String,
    options: HashMap<String, Option<String>>,
}

impl MountEntry {
    fn sourced(mount: MountInfo) -> Option<Self> {
        Some(MountEntry {
            mount_point: unescape_mountinfo_octal(&mount.mount_point),
            fs_type: mount.fs_type,
            source: mount.mount_source?,
            options: mount.super_options,
        })
    }
}

fn unescape_mountinfo_octal(escaped: &Path) -> PathBuf {
    let escaped = escaped.as_os_str().as_bytes();
    let mut unescaped = Vec::with_capacity(escaped.len());
    let mut position = 0;
    while position < escaped.len() {
        let octal_digits = escaped.get(position + 1..position + 4).filter(|digits| {
            escaped[position] == b'\\' && digits.iter().all(|digit| (b'0'..=b'7').contains(digit))
        });
        if let Some(digits) = octal_digits {
            let byte = digits
                .iter()
                .fold(0u32, |value, digit| value * 8 + u32::from(digit - b'0'));
            unescaped.push(u8::try_from(byte).unwrap_or_else(|oversized_escape| {
                tracing::warn!("mounts: octal escape {byte:o} is not a byte: {oversized_escape}");
                b'?'
            }));
            position += 4;
        } else {
            unescaped.push(escaped[position]);
            position += 1;
        }
    }
    PathBuf::from(OsStr::from_bytes(&unescaped))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FilesystemIdentity {
    fs_type: String,
    devices: Vec<KernelDevice>,
}

struct BlockMount {
    identity: FilesystemIdentity,
    mount_point: PathBuf,
    compression: Compression,
}

impl MountEntry {
    fn block_backed(
        self,
        resolve_device: impl Fn(&str) -> Option<KernelDevice>,
    ) -> Option<BlockMount> {
        let mut devices: Vec<KernelDevice> = self
            .source
            .split(MULTI_DEVICE_SEPARATOR)
            .map(resolve_device)
            .collect::<Option<_>>()?;
        let backed_by_virtual_device = devices.iter().any(|device| {
            VIRTUAL_BLOCK_PREFIXES
                .iter()
                .any(|prefix| device.name.starts_with(prefix))
        });
        if devices.is_empty() || backed_by_virtual_device {
            return None;
        }
        devices.sort();
        Some(BlockMount {
            identity: FilesystemIdentity {
                fs_type: self.fs_type,
                devices,
            },
            mount_point: self.mount_point,
            compression: Compression::from(&self.options),
        })
    }
}

fn group_block_mounts(
    entries: impl IntoIterator<Item = MountEntry>,
    resolve_device: impl Fn(&str) -> Option<KernelDevice>,
) -> Vec<FilesystemMount> {
    let by_identity = entries
        .into_iter()
        .filter_map(|entry| entry.block_backed(&resolve_device))
        .fold(BTreeMap::new(), |mut by_identity, mount| {
            by_identity
                .entry(mount.identity.clone())
                .or_insert_with(|| FilesystemMount {
                    fs_type: mount.identity.fs_type,
                    mount_points: Vec::new(),
                    devices: mount.identity.devices,
                    compression: mount.compression,
                })
                .mount_points
                .push(mount.mount_point);
            by_identity
        });
    let mut filesystems: Vec<FilesystemMount> = by_identity
        .into_values()
        .map(FilesystemMount::with_shortest_mount_point_first)
        .collect();
    filesystems.sort_by(|first, second| {
        first
            .primary_mount_point()
            .cmp(second.primary_mount_point())
    });
    filesystems
}

impl FilesystemMount {
    fn with_shortest_mount_point_first(mut self) -> Self {
        self.mount_points.sort_by(|first, second| {
            first
                .as_os_str()
                .len()
                .cmp(&second.as_os_str().len())
                .then_with(|| first.cmp(second))
        });
        self.mount_points.dedup();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KernelDevice {
    pub name: String,
}

impl KernelDevice {
    fn backing(mount_source: &str) -> Option<Self> {
        if !mount_source.starts_with("/dev/") {
            return None;
        }
        let node = fs::canonicalize(mount_source)
            .inspect_err(|resolve_failure| {
                tracing::debug!("mounts: resolving {mount_source} failed: {resolve_failure}");
            })
            .ok()?;
        let device = KernelDevice {
            name: file_name(&node)?,
        };
        device.sysfs_dir().exists().then_some(device)
    }

    #[must_use]
    pub fn sysfs_dir(&self) -> PathBuf {
        Path::new(SYS_CLASS_BLOCK).join(&self.name)
    }

    #[must_use]
    pub fn whole_disk(&self) -> Self {
        let lowest = successors(Some(self.clone()), Self::first_slave)
            .last()
            .unwrap_or_else(|| self.clone());
        lowest.partition_parent().unwrap_or(lowest)
    }

    fn first_slave(&self) -> Option<Self> {
        let mut slaves = list_dir(self.sysfs_dir().join("slaves"));
        slaves.sort();
        Some(KernelDevice {
            name: file_name(slaves.first()?)?,
        })
    }

    fn partition_parent(&self) -> Option<Self> {
        let dir = self.sysfs_dir();
        if !dir.join("partition").exists() {
            return None;
        }
        let resolved = fs::canonicalize(&dir)
            .inspect_err(|resolve_failure| {
                tracing::debug!(
                    "mounts: resolving {} failed: {resolve_failure}",
                    dir.display()
                );
            })
            .ok()?;
        Some(KernelDevice {
            name: file_name(resolved.parent()?)?,
        })
    }
}

fn file_name(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_str()?.to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountRole {
    Root,
    Secondary,
}

#[cfg(test)]
mod block_mount_tests;
