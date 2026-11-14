use std::time::Duration;

use bytesize::ByteSize;
use futures::Stream;
use rustix::fs::{StatVfs, StatVfsMountFlags, statvfs};

use super::{
    Error,
    bcachefs_filesystem_space::{BcachefsFilesystem, BcachefsSpace},
    block_filesystem_mounts::FilesystemMount,
    btrfs_filesystem_space::{BtrfsFilesystem, BtrfsSpace},
    ext4_sysfs_counters::{Ext4Counters, Ext4SysfsDirectory},
    udev_volume_identity::VolumeIdentity,
};
use crate::{SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling};

#[derive(Debug, Clone, PartialEq)]
pub struct FilesystemUsage {
    pub mount: FilesystemMount,
    pub total: ByteSize,
    pub free: ByteSize,
    pub block_size: ByteSize,
    pub access: MountAccess,
    pub volume: VolumeIdentity,
    pub filesystem: Filesystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountAccess {
    ReadWrite,
    ReadOnly,
}

impl From<StatVfsMountFlags> for MountAccess {
    fn from(flags: StatVfsMountFlags) -> Self {
        if flags.contains(StatVfsMountFlags::RDONLY) {
            MountAccess::ReadOnly
        } else {
            MountAccess::ReadWrite
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InodeCount {
    pub total: u64,
    pub free: u64,
}

impl From<&StatVfs> for InodeCount {
    fn from(stat: &StatVfs) -> Self {
        InodeCount {
            total: stat.f_files,
            free: stat.f_favail,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filesystem {
    Btrfs(BtrfsSpace),
    Bcachefs(BcachefsSpace),
    Ext4 {
        counters: Ext4Counters,
        inodes: InodeCount,
    },
    Xfs {
        inodes: InodeCount,
    },
    Other {
        fs_type: String,
    },
}

impl FilesystemUsage {
    #[must_use]
    pub fn used_pct(&self) -> i64 {
        if self.total == ByteSize::default() {
            return 0;
        }
        let used = self.total - self.free;
        ((used.as_u64() as f64 * 100.0) / self.total.as_u64() as f64).round() as i64
    }
}

impl SnapshotSource for FilesystemUsage {
    type Error = Error;
    type StreamInputs = FilesystemMount;

    const NAME: &'static str = "Disk usage";

    fn delivery_stream(
        mount: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(60))
            .blocking_poll_deliveries(move || read_filesystem_usage(mount.clone()))
    }
}

fn read_filesystem_usage(mount: FilesystemMount) -> Result<FilesystemUsage, Error> {
    let stat = read_statvfs(&mount)?;
    Ok(FilesystemUsage {
        total: ByteSize::b(stat.f_blocks * stat.f_frsize),
        free: ByteSize::b(stat.f_bavail * stat.f_frsize),
        block_size: ByteSize::b(stat.f_bsize),
        access: MountAccess::from(stat.f_flag),
        volume: VolumeIdentity::of_devices(&mount.devices),
        filesystem: read_filesystem_space(&mount, &stat)?,
        mount,
    })
}

fn read_filesystem_space(mount: &FilesystemMount, stat: &StatVfs) -> Result<Filesystem, Error> {
    match mount.fs_type.as_str() {
        "btrfs" => Ok(Filesystem::Btrfs(
            BtrfsFilesystem::containing(&mount.devices)?.space(mount.compression.clone())?,
        )),
        "bcachefs" => Ok(Filesystem::Bcachefs(
            BcachefsFilesystem::containing(&mount.devices)?.space()?,
        )),
        "ext4" | "ext3" | "ext2" => Ok(Filesystem::Ext4 {
            counters: Ext4SysfsDirectory::containing(&mount.devices)?.counters()?,
            inodes: InodeCount::from(stat),
        }),
        "xfs" => Ok(Filesystem::Xfs {
            inodes: InodeCount::from(stat),
        }),
        other => Ok(Filesystem::Other {
            fs_type: other.to_owned(),
        }),
    }
}

fn read_statvfs(mount: &FilesystemMount) -> Result<StatVfs, Error> {
    statvfs(mount.primary_mount_point()).map_err(|errno| Error::Statvfs {
        mount_point: mount.primary_mount_point().display().to_string(),
        errno: errno.raw_os_error(),
    })
}
