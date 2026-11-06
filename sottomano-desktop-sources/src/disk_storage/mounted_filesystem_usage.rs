use std::time::Duration;

use bytesize::ByteSize;
use futures::Stream;
use rustix::fs::statvfs;

use super::{
    Error,
    bcachefs_filesystem_space::{BcachefsFilesystem, BcachefsSpace},
    block_filesystem_mounts::FilesystemMount,
    btrfs_filesystem_space::{BtrfsFilesystem, BtrfsSpace},
};
use crate::{SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling};

#[derive(Debug, Clone, PartialEq)]
pub struct FilesystemUsage {
    pub mount: FilesystemMount,
    pub total: ByteSize,
    pub free: ByteSize,
    pub filesystem: Filesystem,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filesystem {
    Btrfs(BtrfsSpace),
    Bcachefs(BcachefsSpace),
    Other { fs_type: String },
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

    const NAME: &'static str = "Filesystem usage";

    fn delivery_stream(
        mount: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(60))
            .blocking_poll_deliveries(move || read_filesystem_usage(mount.clone()))
    }
}

fn read_filesystem_usage(mount: FilesystemMount) -> Result<FilesystemUsage, Error> {
    let filesystem = read_filesystem_space(&mount)?;
    read_statvfs(mount, filesystem)
}

fn read_filesystem_space(mount: &FilesystemMount) -> Result<Filesystem, Error> {
    match mount.fs_type.as_str() {
        "btrfs" => Ok(Filesystem::Btrfs(
            BtrfsFilesystem::containing(&mount.devices)?.space(mount.compression.clone())?,
        )),
        "bcachefs" => Ok(Filesystem::Bcachefs(
            BcachefsFilesystem::containing(&mount.devices)?.space()?,
        )),
        other => Ok(Filesystem::Other {
            fs_type: other.to_owned(),
        }),
    }
}

fn read_statvfs(mount: FilesystemMount, filesystem: Filesystem) -> Result<FilesystemUsage, Error> {
    let stat = statvfs(mount.primary_mount_point()).map_err(|errno| Error::Statvfs {
        mount_point: mount.primary_mount_point().display().to_string(),
        errno: errno.raw_os_error(),
    })?;
    Ok(FilesystemUsage {
        total: ByteSize::b(stat.f_blocks * stat.f_frsize),
        free: ByteSize::b(stat.f_bavail * stat.f_frsize),
        filesystem,
        mount,
    })
}
