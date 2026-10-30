use std::{fs, path::PathBuf, time::Duration};

use bytesize::ByteSize;
use futures::Stream;
use rustix::fs::statvfs;

use super::{
    Error,
    bcachefs_filesystem_space::BcachefsFilesystem,
    block_filesystem_mounts::{FilesystemMount, KernelDevice},
};
use crate::{
    SnapshotDelivery, SnapshotSource, blocking_background_work::blocking_poll,
    delivery_cadence_drivers::PeriodicPolling,
};

const SECTOR_BYTES: u64 = 512;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiskOccupancyInputs {
    pub disk: KernelDevice,
    pub filesystems: Vec<FilesystemMount>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilesystemShare {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub used_on_disk: ByteSize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskSpaceOccupancy {
    pub disk: String,
    pub capacity: ByteSize,
    pub filesystem_shares: Vec<FilesystemShare>,
}

impl DiskSpaceOccupancy {
    #[must_use]
    pub fn used(&self) -> ByteSize {
        ByteSize::b(
            self.filesystem_shares
                .iter()
                .map(|share| share.used_on_disk.as_u64())
                .sum(),
        )
    }

    #[must_use]
    pub fn free(&self) -> ByteSize {
        ByteSize::b(self.capacity.as_u64().saturating_sub(self.used().as_u64()))
    }

    #[must_use]
    pub fn used_pct(&self) -> i64 {
        if self.capacity == ByteSize::default() {
            return 0;
        }
        ((self.used().as_u64() as f64 * 100.0) / self.capacity.as_u64() as f64)
            .round()
            .min(100.0) as i64
    }
}

impl SnapshotSource for DiskSpaceOccupancy {
    type Error = Error;
    type StreamInputs = DiskOccupancyInputs;

    const NAME: &'static str = "Disk space";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(60))
            .poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl DiskSpaceOccupancy {
    async fn poll(inputs: DiskOccupancyInputs) -> Result<Self, Error> {
        blocking_poll(move || read_disk_occupancy(&inputs)).await
    }
}

fn read_disk_occupancy(inputs: &DiskOccupancyInputs) -> Result<DiskSpaceOccupancy, Error> {
    Ok(DiskSpaceOccupancy {
        disk: inputs.disk.name.clone(),
        capacity: block_device_size(&inputs.disk)?,
        filesystem_shares: inputs
            .filesystems
            .iter()
            .filter(|filesystem| filesystem.sits_on_disk(&inputs.disk))
            .map(|filesystem| filesystem_share_on_disk(filesystem, &inputs.disk))
            .collect::<Result<_, _>>()?,
    })
}

fn filesystem_share_on_disk(
    filesystem: &FilesystemMount,
    disk: &KernelDevice,
) -> Result<FilesystemShare, Error> {
    let used_on_disk = match filesystem.fs_type.as_str() {
        "bcachefs" => bcachefs_used_on_disk(filesystem, disk)?,
        _ => proportional_used_on_disk(filesystem, disk)?,
    };
    Ok(FilesystemShare {
        mount_point: filesystem.primary_mount_point().to_path_buf(),
        fs_type: filesystem.fs_type.clone(),
        used_on_disk,
    })
}

fn bcachefs_used_on_disk(
    filesystem: &FilesystemMount,
    disk: &KernelDevice,
) -> Result<ByteSize, Error> {
    let space = BcachefsFilesystem::containing(&filesystem.devices)?.space()?;
    Ok(ByteSize::b(
        space
            .devices
            .iter()
            .filter(|member| {
                &KernelDevice {
                    name: member.partition.clone(),
                }
                .whole_disk()
                    == disk
            })
            .map(|member| {
                member
                    .capacity
                    .as_u64()
                    .saturating_sub(member.free.as_u64())
            })
            .sum(),
    ))
}

fn proportional_used_on_disk(
    filesystem: &FilesystemMount,
    disk: &KernelDevice,
) -> Result<ByteSize, Error> {
    let mount_point = filesystem.primary_mount_point();
    let stat = statvfs(mount_point).map_err(|errno| Error::Statvfs {
        mount_point: mount_point.display().to_string(),
        errno: errno.raw_os_error(),
    })?;
    let used = stat.f_blocks.saturating_sub(stat.f_bfree) * stat.f_frsize;
    let device_sizes = filesystem
        .devices
        .iter()
        .map(|device| Ok((device.whole_disk() == *disk, block_device_size(device)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    let total_size: u64 = device_sizes.iter().map(|(_, size)| size.as_u64()).sum();
    let size_on_disk: u64 = device_sizes
        .iter()
        .filter(|(on_disk, _)| *on_disk)
        .map(|(_, size)| size.as_u64())
        .sum();
    if total_size == 0 {
        return Ok(ByteSize::default());
    }
    Ok(ByteSize::b(
        (used as f64 * size_on_disk as f64 / total_size as f64).round() as u64,
    ))
}

fn block_device_size(device: &KernelDevice) -> Result<ByteSize, Error> {
    let path = device.sysfs_dir().join("size");
    let text =
        fs::read_to_string(&path).map_err(|_| Error::Unreadable(path.display().to_string()))?;
    let sectors: u64 = text
        .trim()
        .parse()
        .map_err(|_| Error::Unparsable(path.display().to_string()))?;
    Ok(ByteSize::b(sectors * SECTOR_BYTES))
}
