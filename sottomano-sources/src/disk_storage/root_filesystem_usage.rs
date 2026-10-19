use std::{mem::zeroed, time::Duration};

use bytesize::ByteSize;
use futures::Stream;

use super::{
    Error,
    btrfs_filesystem_space::{BtrfsFilesystem, BtrfsSpace},
    root_mount_facts::RootMount,
};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_background_work::blocking_poll,
    polled_delivery_stream,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RootFilesystem {
    pub total: ByteSize,
    pub free: ByteSize,
    pub filesystem: Filesystem,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filesystem {
    Btrfs(BtrfsSpace),
    Other { fs_type: String },
}

impl RootFilesystem {
    #[must_use]
    pub fn used_pct(&self) -> i64 {
        if self.total == ByteSize::default() {
            return 0;
        }
        let used = self.total - self.free;
        ((used.as_u64() as f64 * 100.0) / self.total.as_u64() as f64).round() as i64
    }
}

impl ReadingSource for RootFilesystem {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "disk";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(60));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl RootFilesystem {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(read_root_disk).await
    }
}

fn read_root_disk() -> Result<RootFilesystem, Error> {
    read_root_statvfs(read_root_filesystem()?)
}

fn read_root_filesystem() -> Result<Filesystem, Error> {
    let mount = RootMount::current()?;
    if mount.fs_type != "btrfs" {
        return Ok(Filesystem::Other {
            fs_type: mount.fs_type,
        });
    }
    let filesystem = BtrfsFilesystem::containing(&mount.device()?)?;
    Ok(Filesystem::Btrfs(filesystem.space(mount.compression())?))
}

fn read_root_statvfs(filesystem: Filesystem) -> Result<RootFilesystem, Error> {
    let mut stat = unsafe { zeroed::<libc::statvfs>() };
    let rc = unsafe { libc::statvfs(c"/".as_ptr(), &raw mut stat) };
    if rc != 0 {
        return Err(Error::Statvfs(errno()));
    }
    let frsize = stat.f_frsize as u64;
    Ok(RootFilesystem {
        total: ByteSize::b(stat.f_blocks as u64 * frsize),
        free: ByteSize::b(stat.f_bavail as u64 * frsize),
        filesystem,
    })
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
