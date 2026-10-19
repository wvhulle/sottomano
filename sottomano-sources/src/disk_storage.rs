pub mod btrfs_filesystem_space;
pub mod disk_io_activity;
pub mod drive_temperature_health;
pub mod root_filesystem_usage;
pub mod root_mount_facts;

use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::blocking_background_work::Panicked;

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
