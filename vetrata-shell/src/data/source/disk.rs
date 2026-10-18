use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    mem::zeroed,
    time::Duration,
};

use bytesize::ByteSize;
use iced::futures::Stream;

use self::{
    btrfs::{BtrfsFilesystem, BtrfsSpace},
    mount::RootMount,
};
use super::{
    DeliveryCadence, Panicked, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_stream,
};
use crate::widgets::tooltip_body::Section;

pub mod activity;
pub mod btrfs;
pub mod health;
pub mod mount;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Statvfs(i32),
    Unreadable(String),
    Unparsable(String),
    NotABlockDevice(String),
    NoBtrfsFilesystemOn(String),
    Panicked,
}

impl From<Panicked> for Error {
    fn from(Panicked: Panicked) -> Self {
        Error::Panicked
    }
}

impl From<Error> for Section {
    fn from(error: Error) -> Self {
        Section::error(error)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Statvfs(code) => write!(f, "statvfs on / failed (errno {code})"),
            Error::Unreadable(path) => write!(f, "cannot read {path}"),
            Error::Unparsable(path) => write!(f, "unexpected contents in {path}"),
            Error::NotABlockDevice(source) => {
                write!(f, "root is mounted from {source}, not a block device")
            }
            Error::NoBtrfsFilesystemOn(device) => {
                write!(f, "no btrfs filesystem found on {device}")
            }
            Error::Panicked => write!(f, "disk read panicked"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disk {
    pub total: ByteSize,
    pub free: ByteSize,
    pub filesystem: Filesystem,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Filesystem {
    Btrfs(BtrfsSpace),
    Other { fs_type: String },
}

impl Disk {
    #[must_use]
    pub fn used_pct(&self) -> i64 {
        if self.total == ByteSize::default() {
            return 0;
        }
        let used = self.total - self.free;
        ((used.as_u64() as f64 * 100.0) / self.total.as_u64() as f64).round() as i64
    }
}

impl ReadingSource for Disk {
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

impl Disk {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(read_root_disk).await
    }
}

fn read_root_disk() -> Result<Disk, Error> {
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

fn read_root_statvfs(filesystem: Filesystem) -> Result<Disk, Error> {
    let mut stat = unsafe { zeroed::<libc::statvfs>() };
    let rc = unsafe { libc::statvfs(c"/".as_ptr(), &raw mut stat) };
    if rc != 0 {
        return Err(Error::Statvfs(errno()));
    }
    let frsize = stat.f_frsize as u64;
    Ok(Disk {
        total: ByteSize::b(stat.f_blocks as u64 * frsize),
        free: ByteSize::b(stat.f_bavail as u64 * frsize),
        filesystem,
    })
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
