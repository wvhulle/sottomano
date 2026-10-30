pub mod bcachefs_filesystem_space;
pub mod block_filesystem_mounts;
pub mod btrfs_filesystem_space;
pub mod disk_io_activity;
pub mod drive_temperature_health;
pub mod filesystem_device_errors;
pub mod mounted_filesystem_usage;
pub mod whole_disk_space_occupancy;

use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::blocking_background_work::Panicked;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Statvfs {
        mount_point: String,
        errno: i32,
    },
    Unreadable(String),
    Unparsable(String),
    NoFilesystemOn {
        fs_type: &'static str,
        device: String,
    },
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
            Error::Statvfs { mount_point, errno } => {
                write!(f, "statvfs on {mount_point} failed (errno {errno})")
            }
            Error::Unreadable(path) => write!(f, "cannot read {path}"),
            Error::Unparsable(path) => write!(f, "unexpected contents in {path}"),
            Error::NoFilesystemOn { fs_type, device } => {
                write!(f, "no {fs_type} filesystem found on {device}")
            }
            Error::Panicked => write!(f, "disk read panicked"),
        }
    }
}
